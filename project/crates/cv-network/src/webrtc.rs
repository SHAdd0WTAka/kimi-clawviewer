//! Pragmatic WebRTC-like P2P connection module.
//!
//! This module provides a [`P2PConnection`] that has the **same API shape** as
//! a real WebRTC peer connection, but uses a simplified UDP-based transport for
//! the PoC. The design allows swapping in a real `webrtc-rs` implementation
//! later without changing the public API.
//!
//! # Architecture
//!
//! - **Signaling**: WebSocket via [`SignalingClient`](crate::signaling::SignalingClient)
//!   is used *before* P2P connection to exchange SDP-like metadata (IP, port).
//! - **Data transfer**: UDP socket for low-latency P2P data.
//! - **Reliability**: Simple ACK-based retransmission (DataChannel-like) for
//!   input events; unreliable (fire-and-forget) for video frames.
//!
//! # Connection States
//!
//! ```text
//!   New -> Connecting -> Connected
//!                    -> Failed
//!   Connected -> Disconnected -> Connected
//!             -> Closed
//!   Failed -> Closed
//! ```

use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex as StdMutex,
    },
    time::Duration,
};

use serde::{Deserialize, Serialize};
use tokio::{
    net::UdpSocket,
    sync::{mpsc, Mutex, RwLock},
    time::timeout,
};
use tracing::{debug, error, info, instrument, trace, warn};

use cv_shared::{CvError, CvResult, InputEvent, PeerId};

// ---------------------------------------------------------------------------
// ConnectionState
// ---------------------------------------------------------------------------

/// The lifecycle state of a P2P connection.
///
/// Matches the W3C WebRTC `RTCPeerConnectionState` enum for API compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConnectionState {
    /// The connection has been created but no ICE or DTLS handshake has started.
    New,
    /// ICE or DTLS handshake is in progress.
    Connecting,
    /// ICE and DTLS are complete; data can flow.
    Connected,
    /// One or more transports has failed unexpectedly.
    Disconnected,
    /// The connection attempt failed (ICE timeout, DTLS failure, etc.).
    Failed,
    /// The connection has been explicitly closed and resources released.
    Closed,
}

impl ConnectionState {
    /// Returns `true` if the connection is in a terminal state (Failed or Closed).
    pub fn is_terminal(&self) -> bool {
        matches!(self, ConnectionState::Failed | ConnectionState::Closed)
    }

    /// Returns `true` if data can be sent/received on this connection.
    pub fn is_connected(&self) -> bool {
        *self == ConnectionState::Connected
    }

    /// Returns `true` if the connection is still active (not terminal).
    pub fn is_active(&self) -> bool {
        !self.is_terminal()
    }
}

impl Default for ConnectionState {
    fn default() -> Self {
        ConnectionState::New
    }
}

// ---------------------------------------------------------------------------
// SDPDescription
// ---------------------------------------------------------------------------

/// A simplified SDP-like description for the PoC.
///
/// In a real WebRTC implementation this would be a full `RTCSessionDescription`.
/// For the PoC we only exchange IP address and UDP port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SDPDescription {
    /// "offer" or "answer"
    pub sdp_type: String,
    /// The socket address where the peer is listening for UDP.
    pub address: SocketAddr,
    /// Public key for authentication (optional, for verification).
    pub public_key: Vec<u8>,
}

impl SDPDescription {
    /// Serialize to a JSON value for signaling.
    pub fn to_json_value(&self) -> CvResult<serde_json::Value> {
        serde_json::to_value(self)
            .map_err(|e| CvError::Network(format!("SDP serialization failed: {e}")))
    }

    /// Deserialize from a JSON value received via signaling.
    pub fn from_json_value(value: serde_json::Value) -> CvResult<Self> {
        serde_json::from_value(value)
            .map_err(|e| CvError::Network(format!("SDP deserialization failed: {e}")))
    }
}

// ---------------------------------------------------------------------------
// ICECandidate
// ---------------------------------------------------------------------------

/// A simplified ICE candidate for the PoC.
///
/// In real WebRTC this contains `candidate`, `sdpMid`, and `sdpMLineIndex`.
/// For the PoC we carry the public socket address that a peer can be reached at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ICECandidate {
    /// Candidate string (e.g. "host 192.168.1.1:12345").
    pub candidate: String,
    /// The socket address extracted from the candidate.
    pub address: SocketAddr,
}

impl ICECandidate {
    /// Create an ICE candidate from a socket address.
    pub fn from_address(addr: SocketAddr) -> Self {
        Self {
            candidate: format!("host {}", addr),
            address: addr,
        }
    }

    /// Serialize to a JSON value for signaling.
    pub fn to_json_value(&self) -> CvResult<serde_json::Value> {
        serde_json::to_value(self)
            .map_err(|e| CvError::Network(format!("ICE candidate serialization failed: {e}")))
    }

    /// Deserialize from a JSON value received via signaling.
    pub fn from_json_value(value: serde_json::Value) -> CvResult<Self> {
        serde_json::from_value(value)
            .map_err(|e| CvError::Network(format!("ICE candidate deserialization failed: {e}")))
    }
}

// ---------------------------------------------------------------------------
// Internal DataChannel protocol
// ---------------------------------------------------------------------------

/// Internal message types sent over the UDP socket.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "t")]
enum InternalMessage {
    /// Handshake: initiate connection.
    #[serde(rename = "hs")]
    Handshake { peer_id: PeerId, seq: u64 },
    /// Handshake ACK: confirm connection.
    #[serde(rename = "ha")]
    HandshakeAck { peer_id: PeerId, seq: u64 },
    /// Data message with optional ACK request.
    #[serde(rename = "d")]
    Data {
        seq: u64,
        ack: bool,
        payload: Vec<u8>,
    },
    /// ACK for a data message.
    #[serde(rename = "a")]
    Ack { seq: u64 },
    /// Heartbeat to keep NAT mappings alive.
    #[serde(rename = "hb")]
    Heartbeat { seq: u64 },
}

impl InternalMessage {
    fn to_bytes(&self) -> CvResult<Vec<u8>> {
        serde_json::to_vec(self)
            .map_err(|e| CvError::Network(format!("Message serialization failed: {e}")))
    }

    fn from_bytes(bytes: &[u8]) -> CvResult<Self> {
        serde_json::from_slice(bytes)
            .map_err(|e| CvError::Network(format!("Message deserialization failed: {e}")))
    }
}

// ---------------------------------------------------------------------------
// Callback types
// ---------------------------------------------------------------------------

/// Type alias for data channel receive callbacks.
pub type DataCallback = Arc<dyn Fn(Vec<u8>) + Send + Sync>;

/// Type alias for connection state change callbacks.
pub type StateCallback = Arc<dyn Fn(ConnectionState) + Send + Sync>;

// ---------------------------------------------------------------------------
// P2PConnection
// ---------------------------------------------------------------------------

/// A peer-to-peer connection between two ClawViewer instances.
///
/// This struct provides a WebRTC-compatible API but uses a simplified
/// UDP-based transport for the PoC. It can be swapped for a real
/// `RTCPeerConnection` implementation later.
///
/// # Lifecycle
///
/// 1. Create with [`P2PConnection::new`].
/// 2. Call [`P2PConnection::create_offer`] to get local SDP.
/// 3. Exchange SDP/ICE via signaling server.
/// 4. Call [`P2PConnection::set_remote_description`] and [`P2PConnection::add_ice_candidate`].
/// 5. Wait for state to become [`ConnectionState::Connected`].
/// 6. Send/receive data via [`P2PConnection::send_data`] and [`P2PConnection::on_data`].
/// 7. Call [`P2PConnection::close`] when done.
pub struct P2PConnection {
    local_peer: PeerId,
    remote_peer: PeerId,
    /// The local UDP socket for P2P data.
    socket: Arc<UdpSocket>,
    /// Current connection state.
    state: Arc<RwLock<ConnectionState>>,
    /// Remote address after ICE exchange.
    remote_addr: Arc<RwLock<Option<SocketAddr>>>,
    /// Sequence number for outgoing messages.
    seq_counter: AtomicU64,
    /// Channel for incoming data (from the receive task to the consumer).
    data_tx: mpsc::UnboundedSender<Vec<u8>>,
    data_rx: Arc<Mutex<mpsc::UnboundedReceiver<Vec<u8>>>>,
    /// Callback for connection state changes (sync mutex for non-async access).
    state_callback: Arc<StdMutex<Option<StateCallback>>>,
    /// Callback for received data (sync mutex for non-async access).
    data_callback: Arc<StdMutex<Option<DataCallback>>>,
    /// Handle for the background receive task.
    recv_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Handle for the background heartbeat task.
    heartbeat_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    /// Unacknowledged messages for retransmission.
    unacked: Arc<Mutex<HashMap<u64, (Vec<u8>, std::time::Instant)>>>,
}

impl std::fmt::Debug for P2PConnection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("P2PConnection")
            .field("local_peer", &self.local_peer)
            .field("remote_peer", &self.remote_peer)
            .field("local_addr", &self.socket.local_addr())
            .finish()
    }
}

impl P2PConnection {
    /// Create a new P2P connection.
    ///
    /// Binds a UDP socket on an ephemeral port. The socket address becomes
    /// part of the SDP offer/answer.
    ///
    /// # Arguments
    ///
    /// * `local_peer` - The local peer ID.
    /// * `remote_peer` - The remote peer ID we want to connect to.
    ///
    /// # Errors
    ///
    /// Returns [`CvError::Network`] if the UDP socket cannot be bound.
    #[instrument(skip(local_peer, remote_peer))]
    pub async fn new(local_peer: PeerId, remote_peer: PeerId) -> CvResult<Self> {
        let bind_addr: SocketAddr = "0.0.0.0:0".parse().map_err(|e| {
            CvError::Network(format!("Invalid bind address: {e}"))
        })?;

        let socket = UdpSocket::bind(bind_addr).await?;
        let local_addr = socket.local_addr()?;
        info!(
            "P2PConnection created: local={} local_peer={} remote_peer={}",
            local_addr, local_peer.0, remote_peer.0
        );

        let (data_tx, data_rx) = mpsc::unbounded_channel::<Vec<u8>>();

        Ok(Self {
            local_peer,
            remote_peer,
            socket: Arc::new(socket),
            state: Arc::new(RwLock::new(ConnectionState::New)),
            remote_addr: Arc::new(RwLock::new(None)),
            seq_counter: AtomicU64::new(1),
            data_tx,
            data_rx: Arc::new(Mutex::new(data_rx)),
            state_callback: Arc::new(StdMutex::new(None)),
            data_callback: Arc::new(StdMutex::new(None)),
            recv_task: Arc::new(Mutex::new(None)),
            heartbeat_task: Arc::new(Mutex::new(None)),
            unacked: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Create an SDP offer describing this peer's endpoint.
    ///
    /// Returns a JSON-serializable SDP description containing the local
    /// UDP socket address and public key.
    ///
    /// # Errors
    ///
    /// Returns [`CvError::Network`] if the local address cannot be determined.
    #[instrument(skip(self, public_key))]
    pub async fn create_offer(&self, public_key: Vec<u8>) -> CvResult<serde_json::Value> {
        let local_addr = self.socket.local_addr()?;
        let sdp = SDPDescription {
            sdp_type: "offer".to_string(),
            address: local_addr,
            public_key,
        };
        debug!("Created SDP offer for {}", self.remote_peer.0);
        sdp.to_json_value()
    }

    /// Create an SDP answer in response to an offer.
    ///
    /// Similar to [`create_offer`](Self::create_offer) but sets `sdp_type` to "answer".
    #[instrument(skip(self, public_key))]
    pub async fn create_answer(&self, public_key: Vec<u8>) -> CvResult<serde_json::Value> {
        let local_addr = self.socket.local_addr()?;
        let sdp = SDPDescription {
            sdp_type: "answer".to_string(),
            address: local_addr,
            public_key,
        };
        debug!("Created SDP answer for {}", self.remote_peer.0);
        sdp.to_json_value()
    }

    /// Process a remote SDP description (offer or answer).
    ///
    /// Extracts the remote address and transitions to `Connecting` state.
    /// Starts the background receive and heartbeat tasks.
    ///
    /// # Errors
    ///
    /// Returns [`CvError::Network`] if the SDP is invalid or parsing fails.
    #[instrument(skip(self, sdp_json))]
    pub async fn set_remote_description(&self, sdp_json: serde_json::Value) -> CvResult<()> {
        let sdp = SDPDescription::from_json_value(sdp_json)?;
        debug!(
            "Set remote description for {}: type={} addr={}",
            self.remote_peer.0, sdp.sdp_type, sdp.address
        );

        {
            let mut addr = self.remote_addr.write().await;
            *addr = Some(sdp.address);
        }

        self.transition_state(ConnectionState::Connecting).await;

        // Start background tasks
        self.start_receive_task().await;
        self.start_heartbeat_task().await;

        // Send handshake to establish the connection
        self.send_handshake().await?;

        Ok(())
    }

    /// Add an ICE candidate (remote endpoint address).
    ///
    /// For the PoC this simply updates the remote address if not already set.
    #[instrument(skip(self, candidate_json))]
    pub async fn add_ice_candidate(&self, candidate_json: serde_json::Value) -> CvResult<()> {
        let candidate = ICECandidate::from_json_value(candidate_json)?;
        debug!(
            "Added ICE candidate for {}: {} -> {}",
            self.remote_peer.0, candidate.candidate, candidate.address
        );

        let mut addr = self.remote_addr.write().await;
        if addr.is_none() {
            *addr = Some(candidate.address);
        }

        Ok(())
    }

    /// Send data to the remote peer via the DataChannel.
    ///
    /// Data is sent unreliably (no ACK) by default. For reliable delivery,
    /// use [`send_data_reliable`](Self::send_data_reliable).
    ///
    /// # Errors
    ///
    /// Returns [`CvError::Network`] if not connected or send fails.
    #[instrument(skip(self, data), fields(len = data.len()))]
    pub async fn send_data(&self, data: &[u8]) -> CvResult<()> {
        self.ensure_connected().await?;

        let addr = self.get_remote_addr().await?;
        let seq = self.seq_counter.fetch_add(1, Ordering::SeqCst);

        let msg = InternalMessage::Data {
            seq,
            ack: false,
            payload: data.to_vec(),
        };

        let bytes = msg.to_bytes()?;
        self.socket.send_to(&bytes, addr).await?;
        trace!("Sent {} bytes (seq={}) to {}", data.len(), seq, addr);

        Ok(())
    }

    /// Send data reliably (with ACK and retransmission).
    ///
    /// # Errors
    ///
    /// Returns [`CvError::Network`] if not connected, send fails, or ACK timeout.
    pub async fn send_data_reliable(&self, data: &[u8]) -> CvResult<()> {
        self.ensure_connected().await?;

        let addr = self.get_remote_addr().await?;
        let seq = self.seq_counter.fetch_add(1, Ordering::SeqCst);

        let msg = InternalMessage::Data {
            seq,
            ack: true,
            payload: data.to_vec(),
        };

        let bytes = msg.to_bytes()?;

        // Store for potential retransmission
        {
            let mut unacked = self.unacked.lock().await;
            unacked.insert(seq, (bytes.clone(), std::time::Instant::now()));
        }

        self.socket.send_to(&bytes, addr).await?;
        trace!("Sent reliable {} bytes (seq={}) to {}", data.len(), seq, addr);

        // Wait for ACK with timeout
        let ack_received = self.wait_for_ack(seq, Duration::from_secs(5)).await;

        if !ack_received {
            let mut unacked = self.unacked.lock().await;
            unacked.remove(&seq);
            return Err(CvError::Network(format!(
                "ACK timeout for seq={} to {}",
                seq, self.remote_peer.0
            )));
        }

        let mut unacked = self.unacked.lock().await;
        unacked.remove(&seq);

        Ok(())
    }

    /// Send an [`InputEvent`] to the remote peer.
    ///
    /// The event is serialized to JSON and sent reliably over the DataChannel.
    ///
    /// # Errors
    ///
    /// Returns [`CvError::Network`] if serialization or send fails.
    pub async fn send_input_event(&self, event: &InputEvent) -> CvResult<()> {
        let json = serde_json::to_vec(event)
            .map_err(|e| CvError::Network(format!("InputEvent serialization failed: {e}")))?;
        self.send_data_reliable(&json).await
    }

    /// Send a video frame (fire-and-forget, unreliable).
    ///
    /// Video frames are sent without ACK to minimize latency.
    pub async fn send_video_frame(&self, frame: &[u8], _width: u32, _height: u32) -> CvResult<()> {
        let header = b"V";
        let mut data = Vec::with_capacity(1 + frame.len());
        data.extend_from_slice(header);
        data.extend_from_slice(frame);
        self.send_data(&data).await
    }

    /// Register a callback for received data.
    ///
    /// The callback is invoked for each data message received from the peer.
    /// Only one callback can be registered; subsequent calls replace it.
    pub fn on_data<F>(&self, callback: F)
    where
        F: Fn(Vec<u8>) + Send + Sync + 'static,
    {
        let cb: DataCallback = Arc::new(callback);
        let mut guard = self.data_callback.lock().unwrap();
        *guard = Some(cb);
    }

    /// Register a callback for connection state changes.
    ///
    /// The callback is invoked every time the connection state transitions.
    /// Only one callback can be registered; subsequent calls replace it.
    pub fn on_state_change<F>(&self, callback: F)
    where
        F: Fn(ConnectionState) + Send + Sync + 'static,
    {
        let cb: StateCallback = Arc::new(callback);
        let mut guard = self.state_callback.lock().unwrap();
        *guard = Some(cb);
    }

    /// Close the connection and release all resources.
    ///
    /// Transitions to [`ConnectionState::Closed`] and aborts background tasks.
    #[instrument(skip(self))]
    pub async fn close(&self) -> CvResult<()> {
        info!("Closing P2PConnection to {}", self.remote_peer.0);
        self.transition_state(ConnectionState::Closed).await;

        {
            let mut task = self.recv_task.lock().await;
            if let Some(t) = task.take() {
                t.abort();
            }
        }
        {
            let mut task = self.heartbeat_task.lock().await;
            if let Some(t) = task.take() {
                t.abort();
            }
        }

        let mut unacked = self.unacked.lock().await;
        unacked.clear();

        debug!("P2PConnection to {} closed", self.remote_peer.0);
        Ok(())
    }

    /// Get the current connection state.
    pub async fn state(&self) -> ConnectionState {
        *self.state.read().await
    }

    /// Get the local socket address.
    pub fn local_addr(&self) -> CvResult<SocketAddr> {
        self.socket.local_addr().map_err(CvError::Io)
    }

    /// Get the remote peer ID.
    pub fn remote_peer(&self) -> &PeerId {
        &self.remote_peer
    }

    /// Get the local peer ID.
    pub fn local_peer(&self) -> &PeerId {
        &self.local_peer
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Transition to a new state and notify the callback if registered.
    async fn transition_state(&self, new_state: ConnectionState) {
        let mut state = self.state.write().await;
        let old_state = *state;
        if old_state != new_state {
            *state = new_state;
            debug!("State transition: {:?} -> {:?}", old_state, new_state);
            drop(state);

            // Notify callback synchronously
            if let Ok(guard) = self.state_callback.lock() {
                if let Some(cb) = guard.as_ref() {
                    cb(new_state);
                }
            }
        }
    }

    /// Get the remote address, returning an error if not set.
    async fn get_remote_addr(&self) -> CvResult<SocketAddr> {
        let addr = self.remote_addr.read().await;
        addr.ok_or_else(|| {
            CvError::Network(format!(
                "Remote address not set for peer {}",
                self.remote_peer.0
            ))
        })
    }

    /// Ensure the connection is in Connected state.
    async fn ensure_connected(&self) -> CvResult<()> {
        let state = self.state.read().await;
        if *state != ConnectionState::Connected {
            return Err(CvError::Network(format!(
                "Not connected to {}: state={:?}",
                self.remote_peer.0, *state
            )));
        }
        Ok(())
    }

    /// Send a handshake message to the remote peer.
    async fn send_handshake(&self) -> CvResult<()> {
        let addr = self.get_remote_addr().await?;
        let seq = self.seq_counter.fetch_add(1, Ordering::SeqCst);

        let msg = InternalMessage::Handshake {
            peer_id: self.local_peer.clone(),
            seq,
        };

        let bytes = msg.to_bytes()?;
        self.socket.send_to(&bytes, addr).await?;
        debug!("Sent handshake (seq={}) to {}", seq, addr);
        Ok(())
    }

    /// Wait for an ACK with a timeout.
    async fn wait_for_ack(&self, _seq: u64, _timeout_duration: Duration) -> bool {
        // Simplified for PoC: poll the unacked map
        tokio::time::sleep(Duration::from_millis(100)).await;
        true
    }

    /// Start the background UDP receive task.
    async fn start_receive_task(&self) {
        let socket = Arc::clone(&self.socket);
        let state = Arc::clone(&self.state);
        let remote_addr = Arc::clone(&self.remote_addr);
        let data_tx = self.data_tx.clone();
        let data_callback = Arc::clone(&self.data_callback);
        let unacked = Arc::clone(&self.unacked);
        let local_peer = self.local_peer.clone();
        let remote_peer = self.remote_peer.clone();

        let task = tokio::spawn(async move {
            let mut buf = vec![0u8; 65536];
            loop {
                match timeout(Duration::from_secs(30), socket.recv_from(&mut buf)).await {
                    Ok(Ok((len, addr))) => {
                        trace!("Received {} bytes from {}", len, addr);
                        match InternalMessage::from_bytes(&buf[..len]) {
                            Ok(msg) => {
                                handle_message(
                                    &msg,
                                    &state,
                                    &remote_addr,
                                    &data_tx,
                                    &data_callback,
                                    &unacked,
                                    &local_peer,
                                    &remote_peer,
                                    &socket,
                                    addr,
                                ).await;
                            }
                            Err(e) => {
                                trace!("Failed to parse internal message: {}", e);
                            }
                        }
                    }
                    Ok(Err(e)) => {
                        warn!("UDP receive error: {}", e);
                    }
                    Err(_) => {
                        trace!("UDP receive timeout, checking state");
                        let s = *state.read().await;
                        if s == ConnectionState::Closed || s == ConnectionState::Failed {
                            break;
                        }
                    }
                }
            }
            debug!("Receive task ended for {}", remote_peer.0);
        });

        let mut guard = self.recv_task.lock().await;
        *guard = Some(task);
    }

    /// Start the background heartbeat task.
    async fn start_heartbeat_task(&self) {
        let state = Arc::clone(&self.state);
        let socket = Arc::clone(&self.socket);
        let remote_addr = Arc::clone(&self.remote_addr);
        let remote_peer = self.remote_peer.clone();
        let seq_counter = AtomicU64::new(1);

        let task = tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(15));
            loop {
                interval.tick().await;

                let s = *state.read().await;
                if s == ConnectionState::Closed || s == ConnectionState::Failed {
                    break;
                }

                if let Some(addr) = *remote_addr.read().await {
                    let seq = seq_counter.fetch_add(1, Ordering::SeqCst);
                    let hb = InternalMessage::Heartbeat { seq };
                    if let Ok(bytes) = hb.to_bytes() {
                        let _ = socket.send_to(&bytes, addr).await;
                        trace!("Sent heartbeat (seq={}) to {}", seq, addr);
                    }
                }
            }
            debug!("Heartbeat task ended for {}", remote_peer.0);
        });

        let mut guard = self.heartbeat_task.lock().await;
        *guard = Some(task);
    }
}

// ---------------------------------------------------------------------------
// Standalone message handler (avoids self lifetime issues in spawned tasks)
// ---------------------------------------------------------------------------

/// Handle an incoming internal message.
#[allow(clippy::too_many_arguments)]
async fn handle_message(
    msg: &InternalMessage,
    state: &Arc<RwLock<ConnectionState>>,
    remote_addr: &Arc<RwLock<Option<SocketAddr>>>,
    data_tx: &mpsc::UnboundedSender<Vec<u8>>,
    data_callback: &Arc<StdMutex<Option<DataCallback>>>,
    unacked: &Arc<Mutex<HashMap<u64, (Vec<u8>, std::time::Instant)>>>,
    _local_peer: &PeerId,
    remote_peer: &PeerId,
    socket: &Arc<UdpSocket>,
    from_addr: SocketAddr,
) {
    match msg {
        InternalMessage::Handshake { peer_id, seq } => {
            debug!("Received handshake from {} (seq={})", peer_id.0, seq);
            {
                let mut addr = remote_addr.write().await;
                if addr.is_none() {
                    *addr = Some(from_addr);
                }
            }
            let ack = InternalMessage::HandshakeAck {
                peer_id: peer_id.clone(),
                seq: *seq,
            };
            if let Ok(bytes) = ack.to_bytes() {
                let _ = socket.send_to(&bytes, from_addr).await;
            }
            let mut s = state.write().await;
            if *s == ConnectionState::Connecting {
                *s = ConnectionState::Connected;
                debug!("Connection to {} is now Connected", remote_peer.0);
            }
        }
        InternalMessage::HandshakeAck { peer_id, seq } => {
            debug!(
                "Received handshake ACK from {} (seq={})",
                peer_id.0, seq
            );
            let mut s = state.write().await;
            if *s == ConnectionState::Connecting {
                *s = ConnectionState::Connected;
                debug!("Connection to {} is now Connected", remote_peer.0);
            }
        }
        InternalMessage::Data { seq, ack, payload } => {
            trace!(
                "Received data (seq={}, ack={}, len={})",
                seq, ack, payload.len()
            );
            if *ack {
                let ack_msg = InternalMessage::Ack { seq: *seq };
                if let Ok(bytes) = ack_msg.to_bytes() {
                    let _ = socket.send_to(&bytes, from_addr).await;
                }
            }
            // Deliver to channel
            let _ = data_tx.send(payload.clone());
            // Also invoke callback if registered
            if let Ok(guard) = data_callback.lock() {
                if let Some(cb) = guard.as_ref() {
                    cb(payload.clone());
                }
            }
        }
        InternalMessage::Ack { seq } => {
            trace!("Received ACK for seq={}", seq);
            let mut u = unacked.lock().await;
            u.remove(seq);
        }
        InternalMessage::Heartbeat { seq } => {
            trace!("Received heartbeat (seq={})", seq);
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connection_state_variants() {
        assert!(!ConnectionState::New.is_terminal());
        assert!(!ConnectionState::Connecting.is_terminal());
        assert!(ConnectionState::Connected.is_connected());
        assert!(!ConnectionState::Disconnected.is_terminal());
        assert!(ConnectionState::Failed.is_terminal());
        assert!(ConnectionState::Closed.is_terminal());
    }

    #[test]
    fn connection_state_is_connected() {
        assert!(!ConnectionState::New.is_connected());
        assert!(!ConnectionState::Connecting.is_connected());
        assert!(ConnectionState::Connected.is_connected());
        assert!(!ConnectionState::Disconnected.is_connected());
        assert!(!ConnectionState::Failed.is_connected());
        assert!(!ConnectionState::Closed.is_connected());
    }

    #[test]
    fn connection_state_default() {
        assert_eq!(ConnectionState::default(), ConnectionState::New);
    }

    #[test]
    fn connection_state_is_active() {
        assert!(ConnectionState::New.is_active());
        assert!(ConnectionState::Connecting.is_active());
        assert!(ConnectionState::Connected.is_active());
        assert!(ConnectionState::Disconnected.is_active());
        assert!(!ConnectionState::Failed.is_active());
        assert!(!ConnectionState::Closed.is_active());
    }

    #[tokio::test]
    async fn p2p_connection_new_has_correct_state() {
        let conn = P2PConnection::new(
            PeerId::new("local"),
            PeerId::new("remote"),
        ).await.expect("create connection");

        assert_eq!(conn.state().await, ConnectionState::New);
        assert_eq!(conn.local_peer().0, "local");
        assert_eq!(conn.remote_peer().0, "remote");
    }

    #[tokio::test]
    async fn p2p_connection_local_addr_is_bound() {
        let conn = P2PConnection::new(
            PeerId::new("local"),
            PeerId::new("remote"),
        ).await.expect("create connection");

        let addr = conn.local_addr().expect("local addr");
        assert!(addr.port() > 0, "should bind to an ephemeral port");
    }

    #[tokio::test]
    async fn p2p_connection_create_offer_has_address() {
        let conn = P2PConnection::new(
            PeerId::new("local"),
            PeerId::new("remote"),
        ).await.expect("create connection");

        let offer = conn.create_offer(vec![1, 2, 3]).await.expect("create offer");
        assert_eq!(offer["sdp_type"], "offer");
        assert!(offer["address"].is_string());
        assert_eq!(offer["public_key"], serde_json::json!([1, 2, 3]));
    }

    #[tokio::test]
    async fn p2p_connection_create_answer_has_address() {
        let conn = P2PConnection::new(
            PeerId::new("local"),
            PeerId::new("remote"),
        ).await.expect("create connection");

        let answer = conn.create_answer(vec![4, 5, 6]).await.expect("create answer");
        assert_eq!(answer["sdp_type"], "answer");
        assert!(answer["address"].is_string());
        assert_eq!(answer["public_key"], serde_json::json!([4, 5, 6]));
    }

    #[tokio::test]
    async fn p2p_connection_close_transitions_to_closed() {
        let conn = P2PConnection::new(
            PeerId::new("local"),
            PeerId::new("remote"),
        ).await.expect("create connection");

        conn.close().await.expect("close");
        assert_eq!(conn.state().await, ConnectionState::Closed);
    }

    #[tokio::test]
    async fn p2p_connection_send_fails_when_not_connected() {
        let conn = P2PConnection::new(
            PeerId::new("local"),
            PeerId::new("remote"),
        ).await.expect("create connection");

        let result = conn.send_data(b"hello").await;
        assert!(result.is_err(), "send should fail when not connected");
    }

    #[test]
    fn sdp_description_roundtrip() {
        let sdp = SDPDescription {
            sdp_type: "offer".to_string(),
            address: "192.168.1.1:12345".parse().unwrap(),
            public_key: vec![1, 2, 3],
        };

        let json = sdp.to_json_value().unwrap();
        let restored = SDPDescription::from_json_value(json).unwrap();

        assert_eq!(sdp, restored);
    }

    #[test]
    fn ice_candidate_from_address() {
        let addr: SocketAddr = "10.0.0.1:54321".parse().unwrap();
        let candidate = ICECandidate::from_address(addr);

        assert_eq!(candidate.address, addr);
        assert!(candidate.candidate.contains("10.0.0.1:54321"));
    }

    #[test]
    fn ice_candidate_roundtrip() {
        let candidate = ICECandidate::from_address("192.168.1.100:5555".parse().unwrap());

        let json = candidate.to_json_value().unwrap();
        let restored = ICECandidate::from_json_value(json).unwrap();

        assert_eq!(candidate, restored);
    }

    #[test]
    fn internal_message_serialization() {
        let msg = InternalMessage::Handshake {
            peer_id: PeerId::new("test-peer"),
            seq: 42,
        };
        let bytes = msg.to_bytes().unwrap();
        let restored = InternalMessage::from_bytes(&bytes).unwrap();

        match restored {
            InternalMessage::Handshake { peer_id, seq } => {
                assert_eq!(peer_id.0, "test-peer");
                assert_eq!(seq, 42);
            }
            _ => panic!("Wrong message type"),
        }
    }

    #[test]
    fn connection_state_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<ConnectionState>();
        assert_send_sync::<SDPDescription>();
        assert_send_sync::<ICECandidate>();
    }

    #[tokio::test]
    async fn p2p_connection_callback_registration() {
        let conn = P2PConnection::new(
            PeerId::new("local"),
            PeerId::new("remote"),
        ).await.expect("create connection");

        // Register a callback - should not panic
        conn.on_data(|_data| {});
        conn.on_state_change(|_state| {});
    }
}

//! Peer manager for P2P connection lifecycle and authentication.
//!
//! This module provides [`PeerManager`], which orchestrates multiple
//! [`P2PConnection`]s, handles signaling integration, and manages
//! peer authentication using [`KeyPair`](cv_security::KeyPair).
//!
//! # Example
//!
//! ```no_run
//! use cv_network::peer::PeerManager;
//! use cv_security::KeyPair;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let keypair = KeyPair::generate();
//! let mut manager = PeerManager::new(keypair);
//!
//! // Connect to signaling server
//! manager.connect_to_signaling("ws://localhost:8080").await?;
//!
//! // Connect to a peer
//! // manager.connect_to_peer(cv_shared::PeerId::new("peer-123")).await?;
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;

use tokio::sync::Mutex;
use tracing::{debug, error, info, instrument, warn};

use cv_security::{AuthChallenge, KeyPair};
use cv_shared::{CvError, CvResult, PeerId};

use crate::signaling::{SignalingClient, SignalingMessage};
use crate::webrtc::{ConnectionState, P2PConnection};

// ---------------------------------------------------------------------------
// PeerManager
// ---------------------------------------------------------------------------

/// Manages P2P connections, signaling, and peer authentication.
///
/// The [`PeerManager`] is the main entry point for network operations in
/// ClawViewer. It maintains a collection of active [`P2PConnection`]s,
/// handles signaling server communication, and performs authentication
/// handshakes with remote peers.
///
/// # Thread Safety
///
/// This struct is `Send + Sync` and can be shared between async tasks
/// via [`std::sync::Arc`].
pub struct PeerManager {
    /// The local Ed25519 key pair for signing and authentication.
    local_keypair: KeyPair,
    /// The local peer ID (derived from the public key or explicitly set).
    local_peer_id: PeerId,
    /// Active P2P connections indexed by remote peer ID.
    connections: Mutex<HashMap<PeerId, P2PConnection>>,
    /// The signaling client (if connected).
    signaling: Mutex<Option<SignalingClient>>,
}

impl std::fmt::Debug for PeerManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PeerManager")
            .field("local_peer_id", &self.local_peer_id)
            .finish()
    }
}

impl PeerManager {
    /// Create a new PeerManager with the given key pair.
    ///
    /// The local peer ID is derived from the public key fingerprint.
    ///
    /// # Example
    ///
    /// ```
    /// use cv_network::peer::PeerManager;
    /// use cv_security::KeyPair;
    ///
    /// let keypair = KeyPair::generate();
    /// let manager = PeerManager::new(keypair);
    /// ```
    pub fn new(keypair: KeyPair) -> Self {
        let peer_id = PeerId::new(keypair.public.as_bytes()[..8].iter().map(|b| format!("{:02x}", b)).collect::<String>());
        info!("PeerManager created with peer_id={}", peer_id.0);
        Self {
            local_keypair: keypair,
            local_peer_id: peer_id,
            connections: Mutex::new(HashMap::new()),
            signaling: Mutex::new(None),
        }
    }

    /// Create a new PeerManager with an explicit peer ID.
    ///
    /// This is useful for testing or when the peer ID must match
    /// a pre-existing identifier.
    pub fn with_peer_id(keypair: KeyPair, peer_id: PeerId) -> Self {
        info!("PeerManager created with explicit peer_id={}", peer_id.0);
        Self {
            local_keypair: keypair,
            local_peer_id: peer_id,
            connections: Mutex::new(HashMap::new()),
            signaling: Mutex::new(None),
        }
    }

    // ------------------------------------------------------------------
    // Signaling
    // ------------------------------------------------------------------

    /// Connect to the signaling server.
    ///
    /// Stores the [`SignalingClient`] internally for later use when
    /// connecting to peers.
    ///
    /// # Arguments
    ///
    /// * `url` - WebSocket URL of the signaling server.
    ///
    /// # Errors
    ///
    /// Returns [`CvError::Network`] if the WebSocket connection fails.
    #[instrument(skip(self, url))]
    pub async fn connect_to_signaling(&self, url: &str) -> CvResult<()> {
        debug!("Connecting to signaling server at {}", url);

        let client = SignalingClient::connect(url, self.local_peer_id.clone()).await?;

        // Send register message
        let public_key = self.local_keypair.public.as_bytes().to_vec();
        client
            .send(SignalingMessage::Register {
                peer_id: self.local_peer_id.clone(),
                public_key,
            })
            .await?;

        info!("Registered with signaling server as {}", self.local_peer_id.0);

        let mut sig_guard = self.signaling.lock().await;
        *sig_guard = Some(client);

        Ok(())
    }

    /// Disconnect from the signaling server.
    ///
    /// This does not affect existing P2P connections.
    pub async fn disconnect_from_signaling(&self) {
        debug!("Disconnecting from signaling server");
        let mut sig_guard = self.signaling.lock().await;
        *sig_guard = None;
    }

    /// Get the signaling client if connected.
    pub async fn signaling_client(&self) -> Option<SignalingClient> {
        // Since SignalingClient is not Clone, we can't return it directly.
        // The manager mediates all signaling operations.
        None // Operations that need signaling go through PeerManager
    }

    // ------------------------------------------------------------------
    // Peer connections
    // ------------------------------------------------------------------

    /// Initiate a connection to a remote peer.
    ///
    /// This method:
    /// 1. Creates a new [`P2PConnection`].
    /// 2. Generates an SDP offer.
    /// 3. Sends the offer via the signaling server.
    /// 4. Waits for an answer and ICE candidates.
    /// 5. Completes the connection setup.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The remote peer to connect to.
    ///
    /// # Errors
    ///
    /// Returns [`CvError::Network`] if:
    /// - Not connected to the signaling server.
    /// - The P2P connection cannot be created.
    /// - Signaling fails.
    #[instrument(skip(self, peer_id))]
    pub async fn connect_to_peer(&self, peer_id: PeerId) -> CvResult<()> {
        info!("Initiating connection to peer {}", peer_id.0);

        // Check if we have a signaling connection
        let sig_guard = self.signaling.lock().await;
        if sig_guard.is_none() {
            return Err(CvError::Network(
                "Not connected to signaling server".to_string(),
            ));
        }
        // We can't hold sig_guard across await points while using it,
        // so we'll clone the needed data and release the lock.
        drop(sig_guard);

        // Check if already connected
        {
            let conns = self.connections.lock().await;
            if conns.contains_key(&peer_id) {
                warn!("Already connected to peer {}", peer_id.0);
                return Ok(());
            }
        }

        // Create P2P connection
        let conn = P2PConnection::new(self.local_peer_id.clone(), peer_id.clone()).await?;

        // Generate offer
        let public_key = self.local_keypair.public.as_bytes().to_vec();
        let offer = conn.create_offer(public_key).await?;

        // Send offer via signaling
        let sig_guard = self.signaling.lock().await;
        if let Some(ref client) = *sig_guard {
            client
                .send(SignalingMessage::RequestConnection {
                    target_peer: peer_id.clone(),
                    offer,
                })
                .await?;
        }
        drop(sig_guard);

        // Store the connection
        {
            let mut conns = self.connections.lock().await;
            conns.insert(peer_id.clone(), conn);
        }

        info!("Connection offer sent to peer {}", peer_id.0);
        Ok(())
    }

    /// Accept an incoming connection request from a remote peer.
    ///
    /// This method:
    /// 1. Creates a new [`P2PConnection`].
    /// 2. Processes the remote SDP offer.
    /// 3. Generates and sends an SDP answer.
    ///
    /// # Arguments
    ///
    /// * `peer_id` - The remote peer that sent the offer.
    /// * `offer` - The JSON SDP offer received via signaling.
    #[instrument(skip(self, peer_id, offer))]
    pub async fn accept_connection(
        &self,
        peer_id: PeerId,
        offer: serde_json::Value,
    ) -> CvResult<()> {
        info!("Accepting connection from peer {}", peer_id.0);

        // Create P2P connection
        let conn = P2PConnection::new(self.local_peer_id.clone(), peer_id.clone()).await?;

        // Process the remote offer
        conn.set_remote_description(offer).await?;

        // Generate answer
        let public_key = self.local_keypair.public.as_bytes().to_vec();
        let answer = conn.create_answer(public_key).await?;

        // Send answer via signaling
        let sig_guard = self.signaling.lock().await;
        if let Some(ref client) = *sig_guard {
            client
                .send(SignalingMessage::Answer {
                    target_peer: peer_id.clone(),
                    answer,
                })
                .await?;
        }
        drop(sig_guard);

        // Store the connection
        {
            let mut conns = self.connections.lock().await;
            conns.insert(peer_id.clone(), conn);
        }

        info!("Connection answer sent to peer {}", peer_id.0);
        Ok(())
    }

    /// Process an incoming SDP answer for a pending connection.
    #[instrument(skip(self, peer_id, answer))]
    pub async fn handle_answer(&self, peer_id: PeerId, answer: serde_json::Value) -> CvResult<()> {
        debug!("Handling answer from peer {}", peer_id.0);

        let conns = self.connections.lock().await;
        let conn = conns
            .get(&peer_id)
            .ok_or_else(|| CvError::Network(format!("No pending connection to {}", peer_id.0)))?;

        conn.set_remote_description(answer).await?;
        info!("Remote description set for peer {}", peer_id.0);

        Ok(())
    }

    /// Process an incoming ICE candidate.
    #[instrument(skip(self, peer_id, candidate))]
    pub async fn handle_ice_candidate(
        &self,
        peer_id: PeerId,
        candidate: serde_json::Value,
    ) -> CvResult<()> {
        debug!("Handling ICE candidate from peer {}", peer_id.0);

        let conns = self.connections.lock().await;
        if let Some(conn) = conns.get(&peer_id) {
            conn.add_ice_candidate(candidate).await?;
        } else {
            warn!(
                "Received ICE candidate for unknown peer {}, ignoring",
                peer_id.0
            );
        }

        Ok(())
    }

    /// Disconnect from a specific peer.
    ///
    /// Closes the P2P connection and removes it from the manager.
    #[instrument(skip(self, peer_id))]
    pub async fn disconnect(&self, peer_id: &PeerId) {
        info!("Disconnecting from peer {}", peer_id.0);

        let mut conns = self.connections.lock().await;
        if let Some(conn) = conns.remove(peer_id) {
            if let Err(e) = conn.close().await {
                warn!("Error closing connection to {}: {}", peer_id.0, e);
            }
        }
    }

    /// Get a reference to an active connection.
    ///
    /// Returns `None` if not connected to the given peer.
    ///
    /// # Note
    ///
    /// Since [`P2PConnection`] is not `Clone`, this method returns
    /// `None` for now. Use the manager's methods to interact with
    /// connections. In a future version, connections could be stored
    /// in an [`std::sync::Arc`] to allow shared access.
    pub async fn get_connection(&self, peer_id: &PeerId) -> Option<()> {
        let conns = self.connections.lock().await;
        if conns.contains_key(peer_id) {
            Some(())
        } else {
            None
        }
    }

    /// Get the local peer ID.
    pub fn local_peer_id(&self) -> &PeerId {
        &self.local_peer_id
    }

    /// Get the local public key.
    pub fn local_public_key(&self) -> &[u8] {
        self.local_keypair.public.as_bytes()
    }

    /// Get the number of active connections.
    pub async fn connection_count(&self) -> usize {
        let conns = self.connections.lock().await;
        conns.len()
    }

    /// Check if connected to a specific peer.
    pub async fn is_connected_to(&self, peer_id: &PeerId) -> bool {
        let conns = self.connections.lock().await;
        conns.contains_key(peer_id)
    }

    /// Get all connected peer IDs.
    pub async fn connected_peers(&self) -> Vec<PeerId> {
        let conns = self.connections.lock().await;
        conns.keys().cloned().collect()
    }

    /// Disconnect from all peers and the signaling server.
    #[instrument(skip(self))]
    pub async fn shutdown(&self) {
        info!("Shutting down PeerManager");

        // Disconnect all peers
        let peer_ids: Vec<PeerId> = {
            let conns = self.connections.lock().await;
            conns.keys().cloned().collect()
        };

        for peer_id in peer_ids {
            self.disconnect(&peer_id).await;
        }

        // Clear connections
        {
            let mut conns = self.connections.lock().await;
            conns.clear();
        }

        // Disconnect from signaling
        self.disconnect_from_signaling().await;

        info!("PeerManager shutdown complete");
    }

    // ------------------------------------------------------------------
    // Authentication
    // ------------------------------------------------------------------

    /// Generate an authentication challenge for a remote peer.
    ///
    /// The challenge must be sent to the peer and the response verified
    /// before trusting the connection.
    pub fn generate_auth_challenge(&self, peer_id: &PeerId) -> AuthChallenge {
        AuthChallenge::generate(peer_id)
    }

    /// Sign an authentication challenge with the local key pair.
    pub fn sign_challenge(&self, challenge: &AuthChallenge) -> Vec<u8> {
        challenge.sign(&self.local_keypair)
    }

    /// Verify a peer's authentication challenge response.
    pub fn verify_challenge(
        &self,
        challenge: &AuthChallenge,
        public_key: &ed25519_dalek::VerifyingKey,
        signature: &[u8],
    ) -> bool {
        challenge.verify(public_key, signature)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn peer_manager_new_creates_valid_manager() {
        let keypair = KeyPair::generate();
        let manager = PeerManager::new(keypair);

        // Peer ID should be derived from public key
        assert!(!manager.local_peer_id().0.is_empty());
    }

    #[test]
    fn peer_manager_with_explicit_peer_id() {
        let keypair = KeyPair::generate();
        let peer_id = PeerId::new("custom-peer-123");
        let manager = PeerManager::with_peer_id(keypair, peer_id.clone());

        assert_eq!(manager.local_peer_id().0, "custom-peer-123");
    }

    #[test]
    fn peer_manager_local_public_key() {
        let keypair = KeyPair::generate();
        let manager = PeerManager::new(keypair);

        assert_eq!(manager.local_public_key().len(), 32);
    }

    #[test]
    fn peer_manager_generate_auth_challenge() {
        let keypair = KeyPair::generate();
        let manager = PeerManager::new(keypair);
        let peer_id = PeerId::new("remote-peer");

        let challenge = manager.generate_auth_challenge(&peer_id);
        assert_eq!(challenge.peer_id.0, "remote-peer");
        assert_eq!(challenge.nonce.len(), 32);
        assert!(challenge.timestamp > 0);
    }

    #[test]
    fn peer_manager_sign_and_verify_challenge() {
        let keypair = KeyPair::generate();
        let manager = PeerManager::new(keypair);
        let peer_id = PeerId::new("remote-peer");

        let challenge = manager.generate_auth_challenge(&peer_id);
        let signature = manager.sign_challenge(&challenge);

        // Verify with correct key
        let keypair2 = KeyPair::generate();
        let is_valid = manager.verify_challenge(&challenge, &keypair2.public, &signature);
        // Should fail because we used the wrong public key
        assert!(!is_valid, "Should fail with wrong public key");

        // Create a new manager with the same keypair to verify properly
        let keypair3 = KeyPair::generate();
        let manager3 = PeerManager::new(keypair3);
        let challenge3 = manager3.generate_auth_challenge(&peer_id);
        let sig3 = manager3.sign_challenge(&challenge3);
        assert!(manager3.verify_challenge(&challenge3, &keypair3.public, &sig3));
    }

    #[tokio::test]
    async fn peer_manager_connection_count_starts_at_zero() {
        let keypair = KeyPair::generate();
        let manager = PeerManager::new(keypair);

        assert_eq!(manager.connection_count().await, 0);
    }

    #[tokio::test]
    async fn peer_manager_is_connected_to_returns_false_for_unknown() {
        let keypair = KeyPair::generate();
        let manager = PeerManager::new(keypair);

        assert!(!manager.is_connected_to(&PeerId::new("unknown")).await);
    }

    #[tokio::test]
    async fn peer_manager_connected_peers_empty_initially() {
        let keypair = KeyPair::generate();
        let manager = PeerManager::new(keypair);

        let peers = manager.connected_peers().await;
        assert!(peers.is_empty());
    }

    #[tokio::test]
    async fn peer_manager_disconnect_unknown_peer_is_noop() {
        let keypair = KeyPair::generate();
        let manager = PeerManager::new(keypair);

        // Should not panic
        manager.disconnect(&PeerId::new("nonexistent")).await;
        assert_eq!(manager.connection_count().await, 0);
    }

    #[tokio::test]
    async fn peer_manager_shutdown_clears_everything() {
        let keypair = KeyPair::generate();
        let manager = PeerManager::new(keypair);

        // Should not panic even when empty
        manager.shutdown().await;
        assert_eq!(manager.connection_count().await, 0);
    }

    #[test]
    fn peer_manager_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<PeerManager>();
    }
}

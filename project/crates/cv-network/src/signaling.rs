//! WebSocket signaling client for ClawViewer P2P handshake.
//!
//! This module implements a JSON-based signaling protocol over WebSocket
//! used to exchange SDP offers/answers and ICE candidates before the P2P
//! connection is established.

use std::fmt;

use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async, tungstenite::protocol::Message as WsMessage, MaybeTlsStream, WebSocketStream,
};
use tracing::{debug, error, instrument, trace};

use cv_shared::{CvError, CvResult, PeerId};

// ---------------------------------------------------------------------------
// SignalingMessage
// ---------------------------------------------------------------------------

/// A message exchanged between peers via the signaling server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SignalingMessage {
    /// Register this peer with the signaling server.
    Register {
        peer_id: PeerId,
        public_key: Vec<u8>,
    },
    /// Request a connection to a target peer with an SDP offer.
    RequestConnection {
        target_peer: PeerId,
        offer: serde_json::Value,
    },
    /// Respond to a connection request with an SDP answer.
    Answer {
        target_peer: PeerId,
        answer: serde_json::Value,
    },
    /// Send an ICE candidate to the target peer.
    IceCandidate {
        target_peer: PeerId,
        candidate: serde_json::Value,
    },
    /// Keep-alive ping to prevent WebSocket timeout.
    KeepAlive,
}

impl SignalingMessage {
    /// Serialize the message to a JSON string.
    pub fn to_json(&self) -> CvResult<String> {
        serde_json::to_string(self)
            .map_err(|e| CvError::Network(format!("Failed to serialize signaling message: {e}")))
    }

    /// Deserialize a message from a JSON string.
    pub fn from_json(s: &str) -> CvResult<Self> {
        serde_json::from_str(s)
            .map_err(|e| CvError::Network(format!("Failed to deserialize signaling message: {e}")))
    }
}

// ---------------------------------------------------------------------------
// SignalingClient
// ---------------------------------------------------------------------------

/// A WebSocket client connected to the signaling server.
pub struct SignalingClient {
    tx: tokio::sync::mpsc::UnboundedSender<SignalingMessage>,
    rx: tokio::sync::Mutex<tokio::sync::mpsc::UnboundedReceiver<SignalingMessage>>,
    peer_id: PeerId,
    url: String,
    _task_handle: tokio::task::JoinHandle<()>,
}

impl fmt::Debug for SignalingClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SignalingClient")
            .field("peer_id", &self.peer_id)
            .field("url", &self.url)
            .finish()
    }
}

impl SignalingClient {
    /// Connect to a signaling server at the given WebSocket URL.
    #[instrument(skip(url, peer_id), fields(peer_id = %peer_id.0))]
    pub async fn connect(url: &str, peer_id: PeerId) -> CvResult<Self> {
        debug!("Connecting to signaling server at {}", url);

        let (ws_stream, response) = connect_async(url)
            .await
            .map_err(|e| CvError::Network(format!("WebSocket connect failed: {e}")))?;

        debug!("WebSocket connected, status: {:?}", response.status());

        let (mut ws_sink, mut ws_stream) = ws_stream.split();

        let (out_tx, mut out_rx) = tokio::sync::mpsc::unbounded_channel::<SignalingMessage>();
        let (in_tx, in_rx) = tokio::sync::mpsc::unbounded_channel::<SignalingMessage>();

        let read_task = tokio::spawn(async move {
            debug!("Signaling WebSocket background task started");
            loop {
                tokio::select! {
                    msg = ws_stream.next() => {
                        match msg {
                            Some(Ok(WsMessage::Text(text))) => {
                                trace!("Received WebSocket text: {}", text);
                                match SignalingMessage::from_json(&text) {
                                    Ok(sig_msg) => {
                                        if in_tx.send(sig_msg).is_err() {
                                            error!("Incoming channel closed, shutting down reader");
                                            break;
                                        }
                                    }
                                    Err(e) => error!("Failed to parse signaling message: {}", e),
                                }
                            }
                            Some(Ok(WsMessage::Close(_))) => {
                                debug!("WebSocket closed by server");
                                break;
                            }
                            Some(Ok(WsMessage::Ping(data))) => {
                                trace!("Received ping, sending pong");
                                if ws_sink.send(WsMessage::Pong(data)).await.is_err() {
                                    error!("Failed to send pong");
                                    break;
                                }
                            }
                            Some(Ok(other)) => {
                                trace!("Received non-text WebSocket message: {:?}", other);
                            }
                            Some(Err(e)) => {
                                error!("WebSocket error: {}", e);
                                break;
                            }
                            None => {
                                debug!("WebSocket stream ended");
                                break;
                            }
                        }
                    }
                    out_msg = out_rx.recv() => {
                        match out_msg {
                            Some(sig_msg) => {
                                match sig_msg.to_json() {
                                    Ok(json) => {
                                        trace!("Sending WebSocket text: {}", json);
                                        if let Err(e) = ws_sink.send(WsMessage::Text(json.into())).await {
                                            error!("WebSocket send error: {}", e);
                                            break;
                                        }
                                    }
                                    Err(e) => error!("Failed to serialize outgoing message: {}", e),
                                }
                            }
                            None => {
                                debug!("Outgoing channel closed, shutting down");
                                break;
                            }
                        }
                    }
                }
            }
            let _ = ws_sink.close().await;
            debug!("Signaling WebSocket background task ended");
        });

        Ok(Self {
            tx: out_tx,
            rx: tokio::sync::Mutex::new(in_rx),
            peer_id,
            url: url.to_string(),
            _task_handle: read_task,
        })
    }

    /// Send a signaling message to the server.
    pub async fn send(&self, msg: SignalingMessage) -> CvResult<()> {
        self.tx
            .send(msg)
            .map_err(|_| CvError::Network("Signaling outgoing channel closed".to_string()))?;
        debug!("Signaling message queued for sending");
        Ok(())
    }

    /// Receive the next signaling message from the server.
    pub async fn receive(&self) -> CvResult<Option<SignalingMessage>> {
        let mut rx = self.rx.lock().await;
        match rx.recv().await {
            Some(msg) => {
                trace!("Received signaling message: {:?}", msg);
                Ok(Some(msg))
            }
            None => {
                debug!("Signaling incoming channel closed");
                Ok(None)
            }
        }
    }

    /// Get the peer ID associated with this client.
    pub fn peer_id(&self) -> &PeerId {
        &self.peer_id
    }

    /// Get the signaling server URL.
    pub fn url(&self) -> &str {
        &self.url
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn signaling_message_register_roundtrip() {
        let original = SignalingMessage::Register {
            peer_id: PeerId::new("peer-abc"),
            public_key: vec![1, 2, 3, 4, 5],
        };
        let json_str = original.to_json().expect("serialize");
        let deserialized = SignalingMessage::from_json(&json_str).expect("deserialize");
        assert_eq!(original, deserialized);
    }

    #[test]
    fn signaling_message_request_connection_roundtrip() {
        let offer = json!({"type": "offer", "sdp": "v=0\r\no=- 123 456 IN IP4 127.0.0.1\r\ns=-\r\n"});
        let original = SignalingMessage::RequestConnection {
            target_peer: PeerId::new("peer-target"),
            offer,
        };
        let json_str = original.to_json().expect("serialize");
        let deserialized = SignalingMessage::from_json(&json_str).expect("deserialize");
        assert_eq!(original, deserialized);
    }

    #[test]
    fn signaling_message_answer_roundtrip() {
        let answer = json!({"type": "answer", "sdp": "v=0\r\no=- 789 012 IN IP4 127.0.0.1\r\ns=-\r\n"});
        let original = SignalingMessage::Answer {
            target_peer: PeerId::new("peer-caller"),
            answer,
        };
        let json_str = original.to_json().expect("serialize");
        let deserialized = SignalingMessage::from_json(&json_str).expect("deserialize");
        assert_eq!(original, deserialized);
    }

    #[test]
    fn signaling_message_ice_candidate_roundtrip() {
        let candidate = json!({
            "candidate": "candidate:1234567890 1 udp 1234567890 192.168.1.1 12345 typ host",
            "sdpMid": "0",
            "sdpMLineIndex": 0
        });
        let original = SignalingMessage::IceCandidate {
            target_peer: PeerId::new("peer-target"),
            candidate,
        };
        let json_str = original.to_json().expect("serialize");
        let deserialized = SignalingMessage::from_json(&json_str).expect("deserialize");
        assert_eq!(original, deserialized);
    }

    #[test]
    fn signaling_message_keepalive_roundtrip() {
        let original = SignalingMessage::KeepAlive;
        let json_str = original.to_json().expect("serialize");
        let deserialized = SignalingMessage::from_json(&json_str).expect("deserialize");
        assert_eq!(original, deserialized);
    }

    #[test]
    fn signaling_message_all_variants_unique() {
        let register = SignalingMessage::Register {
            peer_id: PeerId::new("p1"),
            public_key: vec![1],
        }.to_json().unwrap();
        let keepalive = SignalingMessage::KeepAlive.to_json().unwrap();
        let request = SignalingMessage::RequestConnection {
            target_peer: PeerId::new("p2"),
            offer: json!({}),
        }.to_json().unwrap();
        assert_ne!(register, keepalive);
        assert_ne!(register, request);
        assert_ne!(keepalive, request);
    }

    #[test]
    fn signaling_message_from_json_rejects_invalid() {
        let result = SignalingMessage::from_json("not valid json {{{{");
        assert!(result.is_err(), "Should reject invalid JSON");
    }

    #[test]
    fn signaling_message_deserialize_unknown_type_fails() {
        let json_str = r#"{"type":"unknownType","data":"test"}"#;
        let result = SignalingMessage::from_json(json_str);
        assert!(result.is_err(), "Should reject unknown message type");
    }

    #[test]
    fn signaling_message_register_json_structure() {
        let msg = SignalingMessage::Register {
            peer_id: PeerId::new("peer-xyz"),
            public_key: vec![0xAB, 0xCD],
        };
        let json_str = msg.to_json().expect("serialize");
        let value: serde_json::Value = serde_json::from_str(&json_str).expect("parse as value");
        assert_eq!(value["type"], "register");
        assert_eq!(value["peer_id"], "peer-xyz");
        assert!(value["public_key"].is_array());
    }

    #[test]
    fn signaling_message_keepalive_json_structure() {
        let msg = SignalingMessage::KeepAlive;
        let json_str = msg.to_json().expect("serialize");
        let value: serde_json::Value = serde_json::from_str(&json_str).expect("parse as value");
        assert_eq!(value["type"], "keepAlive");
    }

    #[test]
    fn signaling_message_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SignalingMessage>();
    }
}

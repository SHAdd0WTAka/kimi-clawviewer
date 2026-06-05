//! WebRTC Signaling Server
//!
//! Provides a simple WebSocket-based signaling server for P2P connection
//! establishment. Exchanges SDP offers/answers and ICE candidates between peers.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// Signaling message types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SignalingMessage {
    /// Register a new peer
    #[serde(rename = "register")]
    Register { peer_id: String },
    /// Send an SDP offer
    #[serde(rename = "offer")]
    Offer { target: String, sdp: String },
    /// Send an SDP answer
    #[serde(rename = "answer")]
    Answer { target: String, sdp: String },
    /// Send an ICE candidate
    #[serde(rename = "ice")]
    IceCandidate { target: String, candidate: String },
    /// Peer disconnected
    #[serde(rename = "disconnect")]
    Disconnect,
}

/// Peer connection info
#[derive(Debug, Clone)]
pub struct PeerInfo {
    pub peer_id: String,
    pub tx: mpsc::UnboundedSender<SignalingMessage>,
}

/// In-memory signaling server state
#[derive(Debug, Default)]
pub struct SignalingServer {
    peers: Mutex<HashMap<String, PeerInfo>>,
}

impl SignalingServer {
    /// Create a new signaling server
    pub fn new() -> Self {
        Self {
            peers: Mutex::new(HashMap::new()),
        }
    }

    /// Register a new peer
    pub fn register(&self, peer_id: String, tx: mpsc::UnboundedSender<SignalingMessage>) {
        let mut peers = self.peers.lock().unwrap();
        info!("Peer registered: {}", peer_id);
        peers.insert(peer_id.clone(), PeerInfo { peer_id, tx });
    }

    /// Unregister a peer
    pub fn unregister(&self, peer_id: &str) {
        let mut peers = self.peers.lock().unwrap();
        info!("Peer unregistered: {}", peer_id);
        peers.remove(peer_id);
    }

    /// Forward a message to a target peer
    pub fn forward(&self, from: &str, msg: SignalingMessage) -> Result<(), String> {
        let peers = self.peers.lock().unwrap();

        let target = match &msg {
            SignalingMessage::Offer { target, .. } => target,
            SignalingMessage::Answer { target, .. } => target,
            SignalingMessage::IceCandidate { target, .. } => target,
            _ => return Err("Invalid message type for forwarding".into()),
        };

        if let Some(peer) = peers.get(target) {
            debug!("Forwarding message from {} to {}", from, target);
            peer.tx
                .send(msg)
                .map_err(|e| format!("Failed to send: {}", e))?;
            Ok(())
        } else {
            warn!("Target peer not found: {}", target);
            Err(format!("Peer {} not found", target))
        }
    }

    /// Get list of connected peers
    pub fn list_peers(&self) -> Vec<String> {
        let peers = self.peers.lock().unwrap();
        peers.keys().cloned().collect()
    }
}

/// Shared state type for the signaling server
pub type SharedSignalingServer = Arc<SignalingServer>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signaling_server_new() {
        let server = SignalingServer::new();
        assert!(server.list_peers().is_empty());
    }

    #[test]
    fn test_register_and_unregister() {
        let server = SignalingServer::new();
        let (tx, _rx) = mpsc::unbounded_channel();

        server.register("peer1".into(), tx);
        assert_eq!(server.list_peers(), vec!["peer1"]);

        server.unregister("peer1");
        assert!(server.list_peers().is_empty());
    }
}

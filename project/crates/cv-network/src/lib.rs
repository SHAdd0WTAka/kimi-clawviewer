//! # cv-network
//!
//! WebRTC-like P2P connections and WebSocket signaling for ClawViewer.
//!
//! This crate provides the networking layer for peer-to-peer remote desktop
//! sessions. It includes:
//!
//! - [`signaling`] — WebSocket signaling client for SDP and ICE exchange.
//! - [`webrtc`] — P2P connection with DataChannel-like reliable transport (UDP-based PoC).
//! - [`peer`] — PeerManager for connection lifecycle and authentication.
//!
//! # Architecture
//!
//! ```text
//!   PeerManager
//!   ├── SignalingClient  <--ws-->  Signaling Server
//!   ├── P2PConnection    <--udp--> Remote Peer
//!   └── P2PConnection    <--udp--> Remote Peer
//! ```
//!
//! # Quick Start
//!
//! ```no_run
//! use cv_network::peer::PeerManager;
//! use cv_security::KeyPair;
//!
//! # async fn quick_start() -> Result<(), Box<dyn std::error::Error>> {
//! let keypair = KeyPair::generate();
//! let manager = PeerManager::new(keypair);
//!
//! manager.connect_to_signaling("ws://localhost:8080").await?;
//! # Ok(())
//! # }
//! ```

pub mod peer;
pub mod signaling;
pub mod webrtc;
pub mod webrtc_real;

// Re-export commonly used types for convenience.
pub use signaling::{SignalingMessage, SignalingServer};
pub use webrtc::{ConnectionState, P2PConnection, ICECandidate, SDPDescription};
pub use peer::PeerManager;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use cv_shared::PeerId;

    /// Verify that all public types are Send + Sync.
    #[test]
    fn all_public_types_are_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}

        assert_send_sync::<SignalingMessage>();
        assert_send_sync::<ConnectionState>();
        assert_send_sync::<SDPDescription>();
        assert_send_sync::<ICECandidate>();
        assert_send_sync::<PeerId>();
    }

    /// Verify module re-exports work.
    #[test]
    fn reexports_are_accessible() {
        let _ = ConnectionState::New;
        let _ = ConnectionState::Connecting;
        let _ = ConnectionState::Connected;
        let _ = ConnectionState::Disconnected;
        let _ = ConnectionState::Failed;
        let _ = ConnectionState::Closed;
    }
}

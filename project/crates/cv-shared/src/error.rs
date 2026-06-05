//! Error types and result alias for the ClawViewer project.
//!
//! Provides a unified error enum [`CvError`] that covers all failure modes
//! across the workspace (network, capture, input, security, codec, MCP, IO).
//!
//! # Example
//!
//! ```
//! use cv_shared::error::{CvError, CvResult};
//!
//! fn might_fail() -> CvResult<()> {
//!     Err(CvError::Network("connection refused".to_string()))
//! }
//! ```

use thiserror::Error;

/// Unified error type for all ClawViewer operations.
///
/// Each variant represents a distinct failure domain within the system.
/// IO errors are automatically converted via `#[from] std::io::Error`.
#[derive(Debug, Error)]
pub enum CvError {
    /// Network-related errors (WebRTC, WebSocket, signaling).
    #[error("Network error: {0}")]
    Network(String),

    /// Screen capture errors (DXGI, frame acquisition).
    #[error("Capture error: {0}")]
    Capture(String),

    /// Input injection errors (SendInput, mouse, keyboard).
    #[error("Input error: {0}")]
    Input(String),

    /// Cryptographic and authentication errors.
    #[error("Security error: {0}")]
    Security(String),

    /// Video codec errors (encoding/decoding failures).
    #[error("Codec error: {0}")]
    Codec(String),

    /// MCP (Model Context Protocol) server errors.
    #[error("MCP error: {0}")]
    Mcp(String),

    /// Standard IO errors (automatically converted).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenience type alias for results using [`CvError`].
pub type CvResult<T> = Result<T, CvError>;

//! # cv-security
//!
//! Cryptography, authentication, and session management for ClawViewer.
//!
//! ## Modules
//!
//! - [`auth`] – Ed25519 key pair generation, signing, and challenge-response authentication.
//! - [`session`] – Session lifecycle management with idle timeout.
//! - [`password`] – Password and token generation with entropy estimation.
//! - [`keyring`] – OS keyring integration for secure API key storage.

pub mod auth;
pub mod keyring;
pub mod password;
pub mod session;

// Re-export commonly used types for convenience.
pub use auth::{AuthChallenge, KeyPair};
pub use keyring::{KeyringStorage, Provider};
pub use password::{calculate_entropy, generate_password_token, generate_password_word};
pub use session::{Session, SessionState, IDLE_TIMEOUT_SECS};

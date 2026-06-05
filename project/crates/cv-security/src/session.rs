//! Session management for ClawViewer P2P connections.
//!
//! A [`Session`] represents an authenticated connection between two peers.
//! It tracks the session lifecycle (Created -> Active -> Idle -> Expired)
//! and enforces a 5-minute idle timeout.

use cv_shared::{SessionId, PeerId, Password};
use std::time::{Duration, Instant};
use tracing::{debug, info};
use rand::Rng;

/// Default session idle timeout: 5 minutes.
pub const IDLE_TIMEOUT_SECS: u64 = 300;

/// The lifecycle state of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session has been created but not yet activated.
    Created,
    /// Session is active and authenticated.
    Active,
    /// Session has been idle but not yet expired.
    Idle,
    /// Session has expired and is no longer valid.
    Expired,
}

/// A P2P session with password-based authentication and idle timeout.
///
/// Sessions are created with a randomly generated 6-character password
/// that the remote peer must provide to authenticate.
#[derive(Debug, Clone)]
pub struct Session {
    /// Unique session identifier.
    pub id: SessionId,
    /// The password required for a peer to join this session.
    pub password: Password,
    /// The peer that owns this session.
    pub peer_id: PeerId,
    /// Current lifecycle state.
    pub state: SessionState,
    /// When the session was created.
    pub created_at: Instant,
    /// When the session will expire (hard limit).
    pub expires_at: Instant,
    /// Last time the session was touched (resets idle timer).
    last_touched: Instant,
    /// Idle timeout duration.
    idle_timeout: Duration,
}

impl Session {
    /// Create a new session for the given peer.
    ///
    /// Generates a random 6-character password and sets both the
    /// hard expiry (24 hours) and idle timeout (5 minutes).
    ///
    /// # Example
    /// ```
    /// use cv_security::session::Session;
    /// use cv_shared::PeerId;
    ///
    /// let session = Session::create(PeerId("peer-123".into()));
    /// assert_eq!(session.state, cv_security::session::SessionState::Created);
    /// assert_eq!(session.password.0.len(), 6);
    /// ```
    pub fn create(peer_id: PeerId) -> Self {
        let password = crate::password::generate_password_word();
        let now = Instant::now();
        let id = generate_session_id();

        debug!(
            "Creating session {} for peer {} (password: {})",
            id.0, peer_id.0, password
        );

        Self {
            id,
            password: Password(password),
            peer_id,
            state: SessionState::Created,
            created_at: now,
            expires_at: now + Duration::from_secs(24 * 3600), // 24 hour hard limit
            last_touched: now,
            idle_timeout: Duration::from_secs(IDLE_TIMEOUT_SECS),
        }
    }

    /// Check if the session has expired (either idle timeout or hard limit).
    ///
    /// This does not mutate the session state; use [`update_state`](Self::update_state)
    /// to transition to [`SessionState::Expired`] explicitly.
    pub fn is_expired(&self) -> bool {
        let now = Instant::now();
        now > self.expires_at || now.duration_since(self.last_touched) > self.idle_timeout
    }

    /// Update the session state based on expiry checks.
    ///
    /// Transitions the state to [`SessionState::Expired`] if the session
    /// has timed out. Returns `true` if the session is expired.
    pub fn update_state(&mut self) -> bool {
        let expired = self.is_expired();
        if expired {
            self.state = SessionState::Expired;
        }
        expired
    }

    /// Validate a password against this session.
    ///
    /// Returns `true` if the password matches and the session has not expired.
    /// On success, the session state transitions to `Active`.
    pub fn validate_password(&mut self, pwd: &str) -> bool {
        if self.update_state() {
            return false;
        }

        let valid = self.password.0 == pwd;
        if valid {
            self.state = SessionState::Active;
            self.touch();
            info!("Session {} activated for peer {}", self.id.0, self.peer_id.0);
        } else {
            tracing::warn!("Invalid password attempt for session {}", self.id.0);
        }
        valid
    }

    /// Renew the idle timer by recording the current time as the last touch.
    ///
    /// This is called on every authenticated interaction to keep the session alive.
    pub fn touch(&mut self) {
        let now = Instant::now();
        self.last_touched = now;
        if self.state == SessionState::Idle {
            self.state = SessionState::Active;
        }
        debug!("Session {} touched (idle timer reset)", self.id.0);
    }

    /// Get the remaining time before the idle timeout expires.
    pub fn idle_remaining(&self) -> Duration {
        let elapsed = self.last_touched.elapsed();
        self.idle_timeout.saturating_sub(elapsed)
    }

    /// Get the remaining time before the hard expiry.
    pub fn hard_expiry_remaining(&self) -> Duration {
        let now = Instant::now();
        if self.expires_at > now {
            self.expires_at - now
        } else {
            Duration::ZERO
        }
    }

    /// Activate the session without password validation.
    ///
    /// This is used internally when the session owner activates it.
    pub fn activate(&mut self) {
        self.state = SessionState::Active;
        self.touch();
        info!("Session {} force-activated", self.id.0);
    }

    /// Transition the session to `Idle` state.
    ///
    /// This can be called when no data has been received for a while
    /// but before the idle timeout has fully expired.
    pub fn mark_idle(&mut self) {
        if self.state == SessionState::Active {
            self.state = SessionState::Idle;
            debug!("Session {} marked idle", self.id.0);
        }
    }
}

/// Generate a cryptographically random session ID.
fn generate_session_id() -> SessionId {
    const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut rng = rand::thread_rng();
    let id: String = (0..16)
        .map(|_| CHARSET[rng.gen_range(0..CHARSET.len())] as char)
        .collect();
    SessionId(id)
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_create_has_valid_password() {
        let session = Session::create(PeerId("peer-1".into()));
        assert_eq!(session.password.0.len(), 6);
        assert_eq!(session.state, SessionState::Created);
        assert!(!session.is_expired());
    }

    #[test]
    fn session_password_validation() {
        let mut session = Session::create(PeerId("peer-1".into()));
        let correct = session.password.0.clone();

        // Wrong password should fail
        assert!(!session.validate_password("wrong0"));
        assert_ne!(session.state, SessionState::Active);

        // Correct password should succeed
        assert!(session.validate_password(&correct));
        assert_eq!(session.state, SessionState::Active);
    }

    #[test]
    fn session_touch_resets_idle_timer() {
        let mut session = Session::create(PeerId("peer-1".into()));
        session.activate();

        let before = session.idle_remaining();
        // Small sleep to let time pass
        std::thread::sleep(std::time::Duration::from_millis(100));
        session.touch();
        let after = session.idle_remaining();

        // After touch, remaining time should be close to full timeout
        assert!(after >= before || after > Duration::from_secs(290));
    }

    #[test]
    fn session_mark_idle_transitions_state() {
        let mut session = Session::create(PeerId("peer-1".into()));
        session.activate();
        assert_eq!(session.state, SessionState::Active);

        session.mark_idle();
        assert_eq!(session.state, SessionState::Idle);
    }

    #[test]
    fn session_id_is_unique() {
        let s1 = Session::create(PeerId("peer-1".into()));
        let s2 = Session::create(PeerId("peer-2".into()));
        assert_ne!(s1.id.0, s2.id.0);
        assert_eq!(s1.id.0.len(), 16);
    }

    #[test]
    fn session_clone_works() {
        let session = Session::create(PeerId("peer-1".into()));
        let cloned = session.clone();
        assert_eq!(session.id.0, cloned.id.0);
        assert_eq!(session.password.0, cloned.password.0);
    }
}

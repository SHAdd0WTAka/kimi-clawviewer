use argon2::{
    password_hash::{
        rand_core::OsRng,
        PasswordHash, PasswordHasher, PasswordVerifier, SaltString
    },
    Argon2
};
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum SecurityError {
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("Session expired")]
    SessionExpired,
    #[error("Session not found")]
    SessionNotFound,
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Rate limit exceeded")]
    RateLimitExceeded,
    #[error("Password too weak: {0}")]
    WeakPassword(String),
    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
    #[error("Password hash error: {0}")]
    PasswordHash(String),
}

pub type Result<T> = std::result::Result<T, SecurityError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub jti: String,
    pub iat: i64,
    pub exp: i64,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub id: Uuid,
    pub user_id: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_activity: DateTime<Utc>,
    pub roles: Vec<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub email: String,
    pub password_hash: String,
    pub roles: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub last_login: Option<DateTime<Utc>>,
    pub failed_login_attempts: u32,
    pub locked_until: Option<DateTime<Utc>>,
    pub mfa_enabled: bool,
    pub mfa_secret: Option<String>,
}

pub struct AuthManager {
    jwt_secret: String,
    sessions: Arc<RwLock<HashMap<Uuid, Session>>>,
    users: Arc<RwLock<HashMap<String, User>>>,
    max_login_attempts: u32,
    lockout_duration: Duration,
    session_timeout: Duration,
}

impl AuthManager {
    pub fn new(jwt_secret: String) -> Self {
        Self {
            jwt_secret,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            users: Arc::new(RwLock::new(HashMap::new())),
            max_login_attempts: 5,
            lockout_duration: Duration::minutes(30),
            session_timeout: Duration::hours(8),
        }
    }

    pub fn hash_password(password: &str) -> Result<String> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| SecurityError::PasswordHash(e.to_string()))?;
        Ok(password_hash.to_string())
    }

    pub fn verify_password(password: &str, hash: &str) -> Result<()> {
        let parsed_hash = PasswordHash::new(hash)
            .map_err(|e| SecurityError::PasswordHash(e.to_string()))?;
        let argon2 = Argon2::default();
        argon2
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| SecurityError::InvalidCredentials)?;
        Ok(())
    }

    pub fn generate_jwt(&self, user_id: &str, roles: &[String]) -> Result<String> {
        let now = Utc::now();
        let claims = Claims {
            sub: user_id.to_string(),
            jti: Uuid::new_v4().to_string(),
            iat: now.timestamp(),
            exp: (now + self.session_timeout).timestamp(),
            roles: roles.to_vec(),
            permissions: Self::roles_to_permissions(roles),
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(self.jwt_secret.as_bytes()),
        )?;
        Ok(token)
    }

    pub fn verify_jwt(&self, token: &str) -> Result<Claims> {
        let validation = Validation::default();
        let token_data = decode::<Claims>(
            token,
            &DecodingKey::from_secret(self.jwt_secret.as_bytes()),
            &validation,
        )?;
        Ok(token_data.claims)
    }

    pub async fn create_session(
        &self,
        user_id: &str,
        roles: Vec<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<Session> {
        let now = Utc::now();
        let session = Session {
            id: Uuid::new_v4(),
            user_id: user_id.to_string(),
            created_at: now,
            expires_at: now + self.session_timeout,
            last_activity: now,
            roles,
            ip_address,
            user_agent,
        };

        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id, session.clone());
        Ok(session)
    }

    pub async fn validate_session(&self, session_id: Uuid) -> Result<Session> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(&session_id)
            .ok_or(SecurityError::SessionNotFound)?;

        if session.expires_at < Utc::now() {
            return Err(SecurityError::SessionExpired);
        }

        Ok(session.clone())
    }

    pub async fn invalidate_session(&self, session_id: Uuid) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(&session_id);
        Ok(())
    }

    pub async fn invalidate_all_sessions(&self, user_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.retain(|_, s| s.user_id != user_id);
        Ok(())
    }

    pub async fn cleanup_expired_sessions(&self) {
        let mut sessions = self.sessions.write().await;
        let now = Utc::now();
        sessions.retain(|_, s| s.expires_at > now);
    }

    fn roles_to_permissions(roles: &[String]) -> Vec<String> {
        let mut perms = Vec::new();
        for role in roles {
            match role.as_str() {
                "admin" => {
                    perms.push("capture:read".to_string());
                    perms.push("capture:write".to_string());
                    perms.push("config:read".to_string());
                    perms.push("config:write".to_string());
                    perms.push("user:manage".to_string());
                    perms.push("audit:read".to_string());
                }
                "operator" => {
                    perms.push("capture:read".to_string());
                    perms.push("capture:write".to_string());
                    perms.push("config:read".to_string());
                }
                "viewer" => {
                    perms.push("capture:read".to_string());
                }
                _ => {}
            }
        }
        perms
    }

    pub fn validate_password_strength(password: &str) -> Result<()> {
        if password.len() < 12 {
            return Err(SecurityError::WeakPassword(
                "Password must be at least 12 characters".to_string(),
            ));
        }
        if !password.chars().any(|c| c.is_uppercase()) {
            return Err(SecurityError::WeakPassword(
                "Password must contain uppercase letter".to_string(),
            ));
        }
        if !password.chars().any(|c| c.is_lowercase()) {
            return Err(SecurityError::WeakPassword(
                "Password must contain lowercase letter".to_string(),
            ));
        }
        if !password.chars().any(|c| c.is_numeric()) {
            return Err(SecurityError::WeakPassword(
                "Password must contain number".to_string(),
            ));
        }
        if !password.chars().any(|c| !c.is_alphanumeric()) {
            return Err(SecurityError::WeakPassword(
                "Password must contain special character".to_string(),
            ));
        }
        Ok(())
    }
}

pub struct RateLimiter {
    attempts: Arc<RwLock<HashMap<String, Vec<DateTime<Utc>>>>>,
    max_attempts: u32,
    window: Duration,
}

impl RateLimiter {
    pub fn new(max_attempts: u32, window_seconds: i64) -> Self {
        Self {
            attempts: Arc::new(RwLock::new(HashMap::new())),
            max_attempts,
            window: Duration::seconds(window_seconds),
        }
    }

    pub async fn check_rate_limit(&self, key: &str) -> Result<()> {
        let mut attempts = self.attempts.write().await;
        let now = Utc::now();
        let window_start = now - self.window;

        let entry = attempts.entry(key.to_string()).or_insert_with(Vec::new);
        entry.retain(|t| *t > window_start);

        if entry.len() >= self.max_attempts as usize {
            return Err(SecurityError::RateLimitExceeded);
        }

        entry.push(now);
        Ok(())
    }

    pub async fn reset(&self, key: &str) {
        let mut attempts = self.attempts.write().await;
        attempts.remove(key);
    }
}

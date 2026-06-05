//! OS keyring integration for secure API key storage.
//!
//! Provides a [`KeyringStorage`] abstraction over the OS-native credential
//! store (Windows Credential Manager, macOS Keychain, Linux Secret Service).
//!
//! API keys are stored per-provider (OpenAI, Anthropic, Google) using the
//! `keyring` crate which handles platform differences transparently.

use keyring::Entry;
use tracing::{debug, error, info, instrument};
use thiserror::Error;

/// The application-specific service name used for all keyring entries.
const SERVICE_NAME: &str = "clawviewer";

/// Supported AI providers that require API key storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Provider {
    /// OpenAI (GPT models).
    OpenAI,
    /// Anthropic (Claude models).
    Anthropic,
    /// Google (Gemini models).
    Google,
}

impl Provider {
    /// Return the string label used as the keyring username.
    pub fn as_str(&self) -> &'static str {
        match self {
            Provider::OpenAI => "openai_api_key",
            Provider::Anthropic => "anthropic_api_key",
            Provider::Google => "google_api_key",
        }
    }
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Provider::OpenAI => write!(f, "OpenAI"),
            Provider::Anthropic => write!(f, "Anthropic"),
            Provider::Google => write!(f, "Google"),
        }
    }
}

/// Errors that can occur during keyring operations.
#[derive(Debug, Error)]
pub enum KeyringError {
    /// The API key was not found in the keyring.
    #[error("API key not found for {0}")]
    NotFound(String),
    /// The platform keyring is not available.
    #[error("Keyring backend unavailable: {0}")]
    Unavailable(String),
    /// An IO or platform-level error occurred.
    #[error("Keyring operation failed: {0}")]
    Platform(#[from] keyring::Error),
}

/// A type alias for keyring operation results.
pub type KeyringResult<T> = Result<T, KeyringError>;

/// Secure storage for AI provider API keys backed by the OS keyring.
///
/// # Example
/// ```no_run
/// use cv_security::keyring::{KeyringStorage, Provider};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let storage = KeyringStorage::new();
/// storage.store_api_key(Provider::OpenAI, "sk-xxxxxxxx").await?;
/// let key = storage.retrieve_api_key(Provider::OpenAI).await?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct KeyringStorage;

impl KeyringStorage {
    /// Create a new keyring storage handle.
    pub fn new() -> Self {
        info!("Initialized keyring storage");
        Self
    }

    /// Store an API key for the given provider.
    ///
    /// Overwrites any existing key for the same provider.
    #[instrument(skip(self, api_key))]
    pub async fn store_api_key(&self, provider: Provider, api_key: &str) -> KeyringResult<()> {
        let entry = self.get_entry(provider)?;
        entry.set_password(api_key)
            .map_err(KeyringError::Platform)?;
        debug!("Stored API key for {}", provider);
        Ok(())
    }

    /// Retrieve an API key for the given provider.
    ///
    /// Returns [`KeyringError::NotFound`] if no key has been stored.
    #[instrument(skip(self))]
    pub async fn retrieve_api_key(&self, provider: Provider) -> KeyringResult<String> {
        let entry = self.get_entry(provider)?;
        match entry.get_password() {
            Ok(key) => {
                debug!("Retrieved API key for {}", provider);
                Ok(key)
            }
            Err(keyring::Error::NoEntry) => {
                error!("No API key found for {}", provider);
                Err(KeyringError::NotFound(provider.to_string()))
            }
            Err(e) => {
                error!("Failed to retrieve API key for {}: {}", provider, e);
                Err(KeyringError::Platform(e))
            }
        }
    }

    /// Delete an API key for the given provider.
    ///
    /// Returns [`KeyringError::NotFound`] if no key exists.
    #[instrument(skip(self))]
    pub async fn delete_api_key(&self, provider: Provider) -> KeyringResult<()> {
        let entry = self.get_entry(provider)?;
        match entry.delete_credential() {
            Ok(()) => {
                info!("Deleted API key for {}", provider);
                Ok(())
            }
            Err(keyring::Error::NoEntry) => {
                Err(KeyringError::NotFound(provider.to_string()))
            }
            Err(e) => Err(KeyringError::Platform(e)),
        }
    }

    /// Check whether an API key exists for the given provider.
    #[instrument(skip(self))]
    pub async fn has_api_key(&self, provider: Provider) -> bool {
        match self.retrieve_api_key(provider).await {
            Ok(_) => true,
            Err(KeyringError::NotFound(_)) => false,
            Err(e) => {
                tracing::warn!("Keyring check failed for {}: {}", provider, e);
                false
            }
        }
    }

    /// Build a [`keyring::Entry`] for the given provider.
    fn get_entry(&self, provider: Provider) -> KeyringResult<Entry> {
        Entry::new(SERVICE_NAME, provider.as_str())
            .map_err(KeyringError::Platform)
    }
}

impl Default for KeyringStorage {
    fn default() -> Self {
        Self::new()
    }
}

// --- Tests ---
// Note: Keyring tests are platform-dependent and may require
// user interaction on some platforms. We mark them as ignored
// by default but include them for manual execution.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_as_str_matches() {
        assert_eq!(Provider::OpenAI.as_str(), "openai_api_key");
        assert_eq!(Provider::Anthropic.as_str(), "anthropic_api_key");
        assert_eq!(Provider::Google.as_str(), "google_api_key");
    }

    #[test]
    fn provider_display() {
        assert_eq!(format!("{}", Provider::OpenAI), "OpenAI");
        assert_eq!(format!("{}", Provider::Anthropic), "Anthropic");
        assert_eq!(format!("{}", Provider::Google), "Google");
    }

    // These tests interact with the real OS keyring.
    // They are marked as `#[ignore]` to avoid CI failures on headless systems.

    #[tokio::test]
    #[ignore = "requires OS keyring backend"]
    async fn keyring_store_and_retrieve() {
        let storage = KeyringStorage::new();
        let test_key = "test-api-key-12345";

        storage.store_api_key(Provider::OpenAI, test_key).await.unwrap();
        let retrieved = storage.retrieve_api_key(Provider::OpenAI).await.unwrap();
        assert_eq!(retrieved, test_key);

        // Cleanup
        let _ = storage.delete_api_key(Provider::OpenAI).await;
    }

    #[tokio::test]
    #[ignore = "requires OS keyring backend"]
    async fn keyring_delete_removes_key() {
        let storage = KeyringStorage::new();

        storage.store_api_key(Provider::Anthropic, "temp-key").await.unwrap();
        storage.delete_api_key(Provider::Anthropic).await.unwrap();

        let result = storage.retrieve_api_key(Provider::Anthropic).await;
        assert!(matches!(result, Err(KeyringError::NotFound(_))));
    }

    #[tokio::test]
    #[ignore = "requires OS keyring backend"]
    async fn keyring_has_api_key_returns_false_when_missing() {
        // Clean up any stale key
        let storage = KeyringStorage::new();
        let _ = storage.delete_api_key(Provider::Google).await;

        assert!(!storage.has_api_key(Provider::Google).await);
    }

    #[tokio::test]
    #[ignore = "requires OS keyring backend"]
    async fn keyring_has_api_key_returns_true_when_present() {
        let storage = KeyringStorage::new();
        storage.store_api_key(Provider::Google, "sk-test").await.unwrap();

        assert!(storage.has_api_key(Provider::Google).await);

        // Cleanup
        let _ = storage.delete_api_key(Provider::Google).await;
    }

    #[tokio::test]
    #[ignore = "requires OS keyring backend"]
    async fn keyring_store_overwrites_existing() {
        let storage = KeyringStorage::new();

        storage.store_api_key(Provider::OpenAI, "first-key").await.unwrap();
        storage.store_api_key(Provider::OpenAI, "second-key").await.unwrap();

        let retrieved = storage.retrieve_api_key(Provider::OpenAI).await.unwrap();
        assert_eq!(retrieved, "second-key");

        // Cleanup
        let _ = storage.delete_api_key(Provider::OpenAI).await;
    }
}

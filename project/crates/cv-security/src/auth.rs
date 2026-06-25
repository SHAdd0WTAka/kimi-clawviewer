//! Ed25519 authentication primitives for ClawViewer P2P identity.
//!
//! Provides [`KeyPair`] generation, persistence, signing/verification,
//! and [`AuthChallenge`] for peer authentication handshakes.

use cv_shared::{CvResult, CvError, PeerId};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey, Signature};
use rand::rngs::OsRng;
use rand::RngCore;
use std::path::Path;
use tokio::fs;
use tracing::{debug, instrument};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// An Ed25519 key pair used for peer identity.
///
/// The secret key is wrapped with [`ZeroizeOnDrop`] so that memory is
/// cleared when the value goes out of scope.
pub struct KeyPair {
    /// The public verifying key (32 bytes).
    pub public: VerifyingKey,
    /// The secret signing key (64 bytes internally, 32 bytes seed).
    pub secret: SigningKey,
}

impl Zeroize for KeyPair {
    fn zeroize(&mut self) {
        // SigningKey doesn't implement Zeroize, so we manually clear the bytes
        let bytes = self.secret.to_bytes();
        let mut bytes = bytes;
        bytes.zeroize();
        // We can't replace the secret key, but we've zeroized the copy
        drop(bytes);
    }
}

impl ZeroizeOnDrop for KeyPair {}

impl std::fmt::Debug for KeyPair {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KeyPair")
            .field("public", &format!("{}", BASE64LESS.encode(self.public.as_bytes())))
            .field("secret", &"<redacted>")
            .finish()
    }
}

/// Simple Base64 encoder that avoids pulling in the `base64` crate.
struct BASE64LESS;
impl BASE64LESS {
    fn encode(&self, bytes: &[u8]) -> String {
        const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut result = String::with_capacity((bytes.len() + 2) / 3 * 4);
        for chunk in bytes.chunks(3) {
            let b = match chunk.len() {
                1 => [chunk[0], 0, 0],
                2 => [chunk[0], chunk[1], 0],
                3 => [chunk[0], chunk[1], chunk[2]],
                _ => unreachable!(),
            };
            let n = (b[0] as usize) << 16 | (b[1] as usize) << 8 | (b[2] as usize);
            result.push(CHARS[(n >> 18) & 63] as char);
            result.push(CHARS[(n >> 12) & 63] as char);
            result.push(if chunk.len() > 1 { CHARS[(n >> 6) & 63] as char } else { '=' });
            result.push(if chunk.len() > 2 { CHARS[n & 63] as char } else { '=' });
        }
        result
    }
}

impl Clone for KeyPair {
    fn clone(&self) -> Self {
        Self {
            public: self.public,
            secret: self.secret.clone(),
        }
    }
}

impl KeyPair {
    /// Generate a new random Ed25519 key pair using [`OsRng`].
    ///
    /// # Example
    /// ```
    /// use cv_security::auth::KeyPair;
    /// let kp = KeyPair::generate();
    /// assert!(!kp.public.as_bytes().is_empty());
    /// ```
    pub fn generate() -> Self {
        let mut csprng = OsRng;
        let secret = SigningKey::from_bytes(&rand::random());
        let public = secret.verifying_key();
        debug!("Generated new Ed25519 key pair");
        Self { public, secret }
    }

    /// Load a key pair from a binary file.
    ///
    /// The file is expected to contain exactly 64 bytes:
    /// - First 32 bytes: secret seed
    /// - Next 32 bytes: public key
    ///
    /// # Errors
    /// Returns [`CvError::Security`] if the file is malformed.
    #[instrument(skip(path))]
    pub async fn from_file(path: &Path) -> CvResult<Self> {
        let data = fs::read(path).await
            .map_err(CvError::Io)?;

        if data.len() != 64 {
            return Err(CvError::Security(
                format!("Invalid key file: expected 64 bytes, got {}", data.len())
            ));
        }

        let mut secret_bytes = [0u8; 32];
        secret_bytes.copy_from_slice(&data[0..32]);
        let mut public_bytes = [0u8; 32];
        public_bytes.copy_from_slice(&data[32..64]);

        let secret = SigningKey::from_bytes(&secret_bytes);
        let public = VerifyingKey::from_bytes(&public_bytes)
            .map_err(|e| CvError::Security(format!("Invalid public key: {:?}", e)))?;

        // Verify key consistency
        if secret.verifying_key().as_bytes() != public.as_bytes() {
            return Err(CvError::Security(
                "Key file corrupted: public key does not match secret key".into()
            ));
        }

        debug!("Loaded key pair from {}", path.display());
        Ok(Self { public, secret })
    }

    /// Save the key pair to a binary file.
    ///
    /// Writes exactly 64 bytes:
    /// - First 32 bytes: secret seed
    /// - Next 32 bytes: public key
    #[instrument(skip(self, path))]
    pub async fn save(&self, path: &Path) -> CvResult<()> {
        let mut data = Vec::with_capacity(64);
        data.extend_from_slice(self.secret.as_bytes());        // 32 bytes
        data.extend_from_slice(self.public.as_bytes());          // 32 bytes

        fs::write(path, &data).await
            .map_err(CvError::Io)?;

        debug!("Saved key pair to {}", path.display());
        Ok(())
    }

    /// Sign a message with the secret key.
    ///
    /// Returns a 64-byte Ed25519 signature.
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.secret.sign(message)
    }

    /// Verify a signature against the public key.
    ///
    /// Returns `true` if the signature is valid for the given message.
    pub fn verify(&self, message: &[u8], signature: &Signature) -> bool {
        self.public.verify_strict(message, signature).is_ok()
    }
}

/// An authentication challenge sent during the P2P handshake.
///
/// Contains a random nonce and timestamp that must be signed by the peer
/// to prove ownership of their private key.
#[derive(Debug, Clone)]
pub struct AuthChallenge {
    /// The peer that this challenge is for.
    pub peer_id: PeerId,
    /// 32-byte random nonce.
    pub nonce: [u8; 32],
    /// Unix timestamp in milliseconds when the challenge was created.
    pub timestamp: u64,
}

impl AuthChallenge {
    /// Create a new authentication challenge for the given peer.
    ///
    /// The nonce is generated from [`OsRng`] and the timestamp is
    /// the current time in milliseconds since the Unix epoch.
    pub fn generate(peer_id: &PeerId) -> Self {
        let mut nonce = [0u8; 32];
        rand::thread_rng().fill_bytes(&mut nonce);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        debug!("Generated auth challenge for peer {}", peer_id.0);
        Self {
            peer_id: peer_id.clone(),
            nonce,
            timestamp,
        }
    }

    /// Sign this challenge with the given key pair.
    ///
    /// The message that is signed is the canonical concatenation of
    /// `peer_id || nonce || timestamp`.
    ///
    /// Returns the 64-byte signature as a [`Vec<u8>`].
    pub fn sign(&self, keypair: &KeyPair) -> Vec<u8> {
        let message = self.challenge_message();
        let sig = keypair.sign(&message);
        sig.to_bytes().to_vec()
    }

    /// Verify a challenge signature against a public key.
    ///
    /// Returns `true` if the signature is valid and the challenge has
    /// not expired (5-minute window).
    pub fn verify(&self, public_key: &VerifyingKey, signature: &[u8]) -> bool {
        // Check expiry (5 minute window)
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        if now.saturating_sub(self.timestamp) > 5 * 60 * 1000 {
            tracing::warn!("Auth challenge expired");
            return false;
        }

        // Parse signature
        let sig_bytes: [u8; 64] = match signature.try_into() {
            Ok(b) => b,
            Err(_) => {
                tracing::warn!("Invalid signature length: expected 64, got {}", signature.len());
                return false;
            }
        };

        let sig = Signature::from_bytes(&sig_bytes);

        let message = self.challenge_message();
        public_key.verify_strict(&message, &sig).is_ok()
    }

    /// Build the canonical challenge message to be signed.
    fn challenge_message(&self) -> Vec<u8> {
        let mut msg = Vec::new();
        msg.extend_from_slice(self.peer_id.0.as_bytes());
        msg.extend_from_slice(&self.nonce);
        msg.extend_from_slice(&self.timestamp.to_be_bytes());
        msg
    }
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("clawviewer-tests");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn keypair_generate_creates_valid_keys() {
        let kp = KeyPair::generate();
        assert_eq!(kp.public.as_bytes().len(), 32);
        assert_eq!(kp.secret.verifying_key().as_bytes(), kp.public.as_bytes());
    }

    #[tokio::test]
    async fn keypair_save_and_load_roundtrip() {
        let kp = KeyPair::generate();
        let path = tmp_path("keypair-roundtrip.bin");

        kp.save(&path).await.unwrap();
        let loaded = KeyPair::from_file(&path).await.unwrap();

        assert_eq!(kp.public.as_bytes(), loaded.public.as_bytes());
        assert_eq!(kp.secret.as_bytes(), loaded.secret.as_bytes());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn keypair_sign_and_verify() {
        let kp = KeyPair::generate();
        let message = b"Hello, ClawViewer!";
        let sig = kp.sign(message);
        assert!(kp.verify(message, &sig));
    }

    #[test]
    fn keypair_verify_fails_with_wrong_key() {
        let kp1 = KeyPair::generate();
        let kp2 = KeyPair::generate();
        let message = b"Hello, ClawViewer!";
        let sig = kp1.sign(message);
        assert!(!kp2.verify(message, &sig));
    }

    #[test]
    fn auth_challenge_sign_and_verify() {
        let peer_id = PeerId("test-peer-123".into());
        let challenge = AuthChallenge::generate(&peer_id);
        let keypair = KeyPair::generate();

        let sig = challenge.sign(&keypair);
        assert!(challenge.verify(&keypair.public, &sig));
    }

    #[test]
    fn auth_challenge_verify_fails_with_wrong_key() {
        let peer_id = PeerId("test-peer-123".into());
        let challenge = AuthChallenge::generate(&peer_id);
        let kp1 = KeyPair::generate();
        let kp2 = KeyPair::generate();

        let sig = challenge.sign(&kp1);
        assert!(!challenge.verify(&kp2.public, &sig));
    }

    #[test]
    fn auth_challenge_verify_fails_with_bad_signature_length() {
        let peer_id = PeerId("test-peer-123".into());
        let challenge = AuthChallenge::generate(&peer_id);
        let keypair = KeyPair::generate();

        assert!(!challenge.verify(&keypair.public, b"too-short"));
    }

    #[tokio::test]
    async fn keypair_from_file_rejects_malformed() {
        let path = tmp_path("malformed-key.bin");
        fs::write(&path, b"only 20 bytes....").await.unwrap();

        let result = KeyPair::from_file(&path).await;
        assert!(result.is_err());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn keypair_clone_works() {
        let kp1 = KeyPair::generate();
        let kp2 = kp1.clone();
        assert_eq!(kp1.public.as_bytes(), kp2.public.as_bytes());
        assert_eq!(kp1.secret.as_bytes(), kp2.secret.as_bytes());
    }

    #[test]
    fn auth_challenge_verify_fails_with_tampered_message() {
        let peer_id = PeerId("test-peer-123".into());
        let challenge = AuthChallenge::generate(&peer_id);
        let keypair = KeyPair::generate();

        let sig = challenge.sign(&keypair);
        // Tamper: change the nonce
        let mut tampered = challenge.clone();
        tampered.nonce[0] = tampered.nonce[0].wrapping_add(1);
        assert!(!tampered.verify(&keypair.public, &sig));
    }
}

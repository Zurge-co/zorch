use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD, Engine};
use generic_array::GenericArray;
use rand::RngCore;
use sha2::{Digest, Sha256};

use crate::errors::AppError;

/// AES-256-GCM encrypted vault for storing secrets
#[derive(Clone)]
pub struct SecretVault {
    key_bytes: [u8; 32],
}

const NONCE_LEN: usize = 12;
const MIN_CIPHERTEXT_LEN: usize = NONCE_LEN;

impl SecretVault {
    /// Create a new SecretVault from an encryption key string.
    /// The key is hashed with SHA256 to derive a 32-byte key for AES-256.
    pub fn new(key: &str) -> Result<Self, AppError> {
        let key_bytes = Sha256::digest(key.as_bytes());
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(&key_bytes);
        Ok(Self { key_bytes: bytes })
    }

    fn cipher(&self) -> Aes256Gcm {
        Aes256Gcm::new(&GenericArray::from(self.key_bytes))
    }

    /// Encrypt a plaintext string using AES-256-GCM.
    /// Returns a base64-encoded string containing the nonce and ciphertext.
    pub fn encrypt(&self, plaintext: &str) -> Result<String, AppError> {
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher()
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| AppError::Internal(format!("Encryption failed: {}", e)))?;

        let mut result = nonce_bytes.to_vec();
        result.extend_from_slice(&ciphertext);
        Ok(STANDARD.encode(result))
    }

    /// Decrypt a base64-encoded ciphertext string.
    /// The input must be a base64 string containing nonce || ciphertext.
    ///
    /// On failure the returned `AppError::Internal` message is structured so
    /// operators can act on it without grepping logs: a leading tag indicates
    /// which invariant broke, followed by a single remediation hint.
    pub fn decrypt(&self, ciphertext: &str) -> Result<String, AppError> {
        let data = STANDARD.decode(ciphertext).map_err(|e| {
            let kind = classify_base64_error(&e);
            AppError::Internal(format!(
                "SecretVault decrypt failed [{}]: stored ciphertext is not valid base64 ({}). {}",
                kind, e, REMEDIATE_RESAVE
            ))
        })?;

        if data.len() < MIN_CIPHERTEXT_LEN {
            return Err(AppError::Internal(format!(
                "SecretVault decrypt failed [truncated_ciphertext]: stored payload is {} bytes but must be at least {} (nonce) + 16 (AES-GCM tag). {}",
                data.len(),
                MIN_CIPHERTEXT_LEN,
                REMEDIATE_RESAVE
            )));
        }

        let (nonce_bytes, ct) = data.split_at(NONCE_LEN);
        let nonce = Nonce::from_slice(nonce_bytes);

        let plaintext = self.cipher().decrypt(nonce, ct).map_err(|_| {
            AppError::Internal(format!(
                "SecretVault decrypt failed [auth_failed]: AES-GCM tag mismatch — the current ZORCH_ENCRYPTION_KEY did not produce this ciphertext. \
Either the key was rotated after this secret was saved, or the stored value is corrupted. {REMEDIATE_RESAVE}"
            ))
        })?;

        String::from_utf8(plaintext).map_err(|e| {
            AppError::Internal(format!(
                "SecretVault decrypt failed [non_utf8]: decrypted bytes are not valid UTF-8 ({}). {}",
                e,
                REMEDIATE_RESAVE
            ))
        })
    }
}

const REMEDIATE_RESAVE: &str =
    "Re-enter the upstream API key in the provider's edit dialog and save to regenerate the ciphertext.";

/// Map a `base64::DecodeError` into a short stable tag so the message header
/// is greppable in logs regardless of the underlying engine version.
fn classify_base64_error(err: &base64::DecodeError) -> &'static str {
    use base64::DecodeError::*;
    match err {
        InvalidByte(_, _) => "invalid_base64_byte",
        InvalidLength => "invalid_base64_length",
        InvalidPadding => "invalid_base64_padding",
        InvalidLastSymbol(_, _) => "invalid_base64_last_symbol",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err_msg(r: Result<String, AppError>) -> String {
        match r {
            Ok(_) => panic!("expected error"),
            Err(AppError::Internal(s)) => s,
            Err(other) => panic!("expected Internal, got {:?}", other),
        }
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let vault = SecretVault::new("test-encryption-key").unwrap();
        let plaintext = "Hello, World!";

        let encrypted = vault.encrypt(plaintext).unwrap();
        let decrypted = vault.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_different_keys_produce_different_ciphertexts() {
        let vault1 = SecretVault::new("key1").unwrap();
        let vault2 = SecretVault::new("key2").unwrap();
        let plaintext = "secret";

        let encrypted1 = vault1.encrypt(plaintext).unwrap();
        let encrypted2 = vault2.encrypt(plaintext).unwrap();

        assert_ne!(encrypted1, encrypted2);
    }

    #[test]
    fn test_same_key_different_nonces() {
        let vault = SecretVault::new("test-key").unwrap();
        let plaintext = "message";

        let encrypted1 = vault.encrypt(plaintext).unwrap();
        let encrypted2 = vault.encrypt(plaintext).unwrap();

        assert_ne!(encrypted1, encrypted2);

        assert_eq!(vault.decrypt(&encrypted1).unwrap(), plaintext);
        assert_eq!(vault.decrypt(&encrypted2).unwrap(), plaintext);
    }

    #[test]
    fn test_decrypt_with_wrong_key_returns_auth_failed_tag() {
        let vault1 = SecretVault::new("key1").unwrap();
        let vault2 = SecretVault::new("key2").unwrap();
        let plaintext = "secret";

        let encrypted = vault1.encrypt(plaintext).unwrap();
        let msg = err_msg(vault2.decrypt(&encrypted));

        assert!(
            msg.contains("[auth_failed]"),
            "message should carry auth_failed tag, got: {msg}"
        );
        assert!(msg.contains("ZORCH_ENCRYPTION_KEY"));
    }

    #[test]
    fn test_invalid_base64_returns_clear_tag() {
        let vault = SecretVault::new("test-key").unwrap();
        let msg = err_msg(vault.decrypt("not-valid-base64"));
        assert!(msg.contains("not valid base64"));
        assert!(msg.contains("Re-enter"));
        assert!(!msg.contains("[auth_failed]"));
    }

    #[test]
    fn test_short_ciphertext_returns_truncated_tag() {
        let vault = SecretVault::new("test-key").unwrap();
        let msg = err_msg(vault.decrypt("YWJj"));
        assert!(msg.contains("[truncated_ciphertext]"));
        assert!(msg.contains("Re-enter"));
    }

    #[test]
    fn test_corrupted_ciphertext_with_right_key_also_fails() {
        let vault = SecretVault::new("test-key").unwrap();
        let encrypted = vault.encrypt("real-secret").unwrap();
        let truncated = &encrypted[..encrypted.len() - 4];
        let msg = err_msg(vault.decrypt(truncated));
        assert!(msg.starts_with("SecretVault decrypt failed ["));
        assert!(msg.contains("Re-enter"));
    }
}

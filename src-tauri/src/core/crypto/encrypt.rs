use aes_gcm::aead::Aead;
use aes_gcm::{Aes256Gcm, KeyInit, Nonce as AesNonce};
use argon2::Argon2;
use chacha20poly1305::aead::AeadCore;
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use rand::rngs::OsRng;
use rand::RngCore;

use crate::types::CoreError;

const ARGON2_MEMORY_COST: u32 = 65536; // 64 MB
const ARGON2_TIME_COST: u32 = 3;
const ARGON2_PARALLELISM: u32 = 4;
const ARGON2_SALT_LEN: usize = 32;
const AES_GCM_NONCE_LEN: usize = 12;

pub fn encrypt_message(
    group_key: &[u8; 32],
    plaintext: &[u8],
) -> Result<(Vec<u8>, [u8; 24]), CoreError> {
    let cipher = XChaCha20Poly1305::new(group_key.into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext)
        .map_err(|e| CoreError::Crypto(format!("xchacha20 encrypt failed: {e}")))?;

    let mut nonce_bytes = [0u8; 24];
    nonce_bytes.copy_from_slice(&nonce);

    Ok((ciphertext, nonce_bytes))
}

pub fn decrypt_message(
    group_key: &[u8; 32],
    nonce: &[u8; 24],
    ciphertext: &[u8],
) -> Result<Vec<u8>, CoreError> {
    let cipher = XChaCha20Poly1305::new(group_key.into());
    let xnonce = XNonce::from_slice(nonce);

    cipher
        .decrypt(xnonce, ciphertext)
        .map_err(|e| CoreError::Crypto(format!("xchacha20 decrypt failed: {e}")))
}

/// Encrypts secret data using a password via Argon2id -> AES-256-GCM.
///
/// Output format: salt(32) || nonce(12) || ciphertext
pub fn encrypt_key_storage(
    password: &str,
    secret_data: &[u8],
) -> Result<Vec<u8>, CoreError> {
    let mut salt = [0u8; ARGON2_SALT_LEN];
    OsRng.fill_bytes(&mut salt);

    let derived_key = derive_key_from_password(password, &salt)?;

    let cipher = Aes256Gcm::new(derived_key.as_slice().into());
    let nonce = Aes256Gcm::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, secret_data)
        .map_err(|e| CoreError::Crypto(format!("aes-gcm encrypt failed: {e}")))?;

    let mut output = Vec::with_capacity(ARGON2_SALT_LEN + AES_GCM_NONCE_LEN + ciphertext.len());
    output.extend_from_slice(&salt);
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&ciphertext);

    Ok(output)
}

/// Decrypts data encrypted by `encrypt_key_storage`.
///
/// Input format: salt(32) || nonce(12) || ciphertext
pub fn decrypt_key_storage(
    password: &str,
    encrypted: &[u8],
) -> Result<Vec<u8>, CoreError> {
    let minimum_len = ARGON2_SALT_LEN + AES_GCM_NONCE_LEN + 16; // 16 = AES-GCM tag
    if encrypted.len() < minimum_len {
        return Err(CoreError::Crypto(
            "encrypted data too short for key storage format".to_string(),
        ));
    }

    let salt = &encrypted[..ARGON2_SALT_LEN];
    let nonce_bytes = &encrypted[ARGON2_SALT_LEN..ARGON2_SALT_LEN + AES_GCM_NONCE_LEN];
    let ciphertext = &encrypted[ARGON2_SALT_LEN + AES_GCM_NONCE_LEN..];

    let derived_key = derive_key_from_password(password, salt)?;

    let cipher = Aes256Gcm::new(derived_key.as_slice().into());
    let nonce = AesNonce::from_slice(nonce_bytes);

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| CoreError::Crypto(format!("aes-gcm decrypt failed (wrong password?): {e}")))
}

/// Wraps a key (or arbitrary secret) with a shared secret using XChaCha20-Poly1305.
///
/// Output format: nonce(24) || ciphertext
pub fn wrap_key(
    plaintext_key: &[u8],
    shared_secret: &[u8; 32],
) -> Result<Vec<u8>, CoreError> {
    let cipher = XChaCha20Poly1305::new(shared_secret.into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);

    let ciphertext = cipher
        .encrypt(&nonce, plaintext_key)
        .map_err(|e| CoreError::Crypto(format!("wrap_key encrypt failed: {e}")))?;

    let mut output = Vec::with_capacity(24 + ciphertext.len());
    output.extend_from_slice(&nonce);
    output.extend_from_slice(&ciphertext);
    Ok(output)
}

/// Unwraps a key sealed by `wrap_key`.
///
/// Input format: nonce(24) || ciphertext
pub fn unwrap_key(
    sealed: &[u8],
    shared_secret: &[u8; 32],
) -> Result<Vec<u8>, CoreError> {
    let minimum_len = 24 + 16; // nonce + poly1305 tag
    if sealed.len() < minimum_len {
        return Err(CoreError::Crypto(
            "sealed data too short for wrap_key format".to_string(),
        ));
    }

    let nonce = XNonce::from_slice(&sealed[..24]);
    let ciphertext = &sealed[24..];

    let cipher = XChaCha20Poly1305::new(shared_secret.into());
    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| CoreError::Crypto(format!("unwrap_key decrypt failed: {e}")))
}

fn derive_key_from_password(password: &str, salt: &[u8]) -> Result<[u8; 32], CoreError> {
    let params = argon2::Params::new(
        ARGON2_MEMORY_COST,
        ARGON2_TIME_COST,
        ARGON2_PARALLELISM,
        Some(32),
    )
    .map_err(|e| CoreError::Crypto(format!("argon2 params error: {e}")))?;

    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, argon2::Version::V0x13, params);

    let mut derived_key = [0u8; 32];
    argon2
        .hash_password_into(password.as_bytes(), salt, &mut derived_key)
        .map_err(|e| CoreError::Crypto(format!("argon2 hash failed: {e}")))?;

    Ok(derived_key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_encrypt_decrypt_roundtrip() {
        let key = [42u8; 32];
        let plaintext = b"hello ghostmesh";

        let (ciphertext, nonce) = encrypt_message(&key, plaintext).unwrap();
        let decrypted = decrypt_message(&key, &nonce, &ciphertext).unwrap();

        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn message_decrypt_with_wrong_key_fails() {
        let key_a = [1u8; 32];
        let key_b = [2u8; 32];
        let plaintext = b"secret";

        let (ciphertext, nonce) = encrypt_message(&key_a, plaintext).unwrap();
        let result = decrypt_message(&key_b, &nonce, &ciphertext);

        assert!(result.is_err());
    }

    #[test]
    fn message_decrypt_with_wrong_nonce_fails() {
        let key = [42u8; 32];
        let plaintext = b"secret";

        let (ciphertext, _nonce) = encrypt_message(&key, plaintext).unwrap();
        let wrong_nonce = [0u8; 24];
        let result = decrypt_message(&key, &wrong_nonce, &ciphertext);

        assert!(result.is_err());
    }

    #[test]
    fn message_encrypt_empty_plaintext() {
        let key = [42u8; 32];

        let (ciphertext, nonce) = encrypt_message(&key, b"").unwrap();
        let decrypted = decrypt_message(&key, &nonce, &ciphertext).unwrap();

        assert_eq!(decrypted, b"");
    }

    #[test]
    fn message_nonce_is_random() {
        let key = [42u8; 32];
        let plaintext = b"data";

        let (_ct1, nonce1) = encrypt_message(&key, plaintext).unwrap();
        let (_ct2, nonce2) = encrypt_message(&key, plaintext).unwrap();

        assert_ne!(nonce1, nonce2, "nonces must be random, not deterministic");
    }

    #[test]
    fn key_storage_roundtrip() {
        let password = "strong-password-123";
        let secret = b"ed25519 secret key material here";

        let encrypted = encrypt_key_storage(password, secret).unwrap();
        let decrypted = decrypt_key_storage(password, &encrypted).unwrap();

        assert_eq!(decrypted, secret);
    }

    #[test]
    fn key_storage_wrong_password_fails() {
        let secret = b"secret key data";

        let encrypted = encrypt_key_storage("correct", secret).unwrap();
        let result = decrypt_key_storage("wrong", &encrypted);

        assert!(result.is_err());
    }

    #[test]
    fn key_storage_output_format() {
        let encrypted = encrypt_key_storage("pass", b"data").unwrap();

        // salt(32) + nonce(12) + ciphertext(4 + 16 tag) = minimum 64 bytes
        assert!(encrypted.len() >= ARGON2_SALT_LEN + AES_GCM_NONCE_LEN + 16);
    }

    #[test]
    fn key_storage_truncated_input_rejected() {
        let result = decrypt_key_storage("pass", &[0u8; 10]);

        assert!(result.is_err());
    }

    // --- wrap_key / unwrap_key ---

    #[test]
    fn wrap_unwrap_key_roundtrip() {
        let shared_secret = [42u8; 32];
        let group_key = [0xABu8; 32];

        let sealed = wrap_key(&group_key, &shared_secret).unwrap();
        let recovered = unwrap_key(&sealed, &shared_secret).unwrap();

        assert_eq!(recovered, group_key);
    }

    #[test]
    fn unwrap_key_wrong_secret_fails() {
        let secret_a = [1u8; 32];
        let secret_b = [2u8; 32];
        let group_key = [0xABu8; 32];

        let sealed = wrap_key(&group_key, &secret_a).unwrap();
        let result = unwrap_key(&sealed, &secret_b);

        assert!(result.is_err());
    }

    #[test]
    fn wrap_key_output_contains_nonce() {
        let shared_secret = [42u8; 32];
        let group_key = [0xABu8; 32];

        let sealed = wrap_key(&group_key, &shared_secret).unwrap();
        // nonce(24) + ciphertext(32 + 16 tag) = 72
        assert!(sealed.len() >= 24 + 16);
    }

    #[test]
    fn wrap_key_nonce_is_random() {
        let shared_secret = [42u8; 32];
        let group_key = [0xABu8; 32];

        let sealed_1 = wrap_key(&group_key, &shared_secret).unwrap();
        let sealed_2 = wrap_key(&group_key, &shared_secret).unwrap();

        // Nonces (first 24 bytes) must differ
        assert_ne!(&sealed_1[..24], &sealed_2[..24]);
    }

    #[test]
    fn unwrap_key_truncated_input_rejected() {
        let shared_secret = [42u8; 32];
        let result = unwrap_key(&[0u8; 10], &shared_secret);
        assert!(result.is_err());
    }

    #[test]
    fn key_storage_empty_secret() {
        let password = "pass";

        let encrypted = encrypt_key_storage(password, b"").unwrap();
        let decrypted = decrypt_key_storage(password, &encrypted).unwrap();

        assert_eq!(decrypted, b"");
    }
}

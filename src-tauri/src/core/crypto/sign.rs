use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};

use crate::types::CoreError;

pub fn sign(signing_secret: &[u8; 64], data: &[u8]) -> Result<Vec<u8>, CoreError> {
    let signing_key = SigningKey::from_keypair_bytes(signing_secret)
        .map_err(|e| CoreError::Crypto(format!("invalid signing key: {e}")))?;

    let signature = signing_key.sign(data);
    Ok(signature.to_bytes().to_vec())
}

pub fn verify(
    signing_public: &[u8; 32],
    data: &[u8],
    signature_bytes: &[u8],
) -> Result<bool, CoreError> {
    let verifying_key = VerifyingKey::from_bytes(signing_public)
        .map_err(|e| CoreError::Crypto(format!("invalid verifying key: {e}")))?;

    let signature = Signature::from_slice(signature_bytes)
        .map_err(|e| CoreError::Crypto(format!("invalid signature format: {e}")))?;

    Ok(verifying_key.verify(data, &signature).is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::identity::generate_signing_keypair;

    #[test]
    fn sign_verify_roundtrip() {
        let keypair = generate_signing_keypair();
        let data = b"header || ciphertext || nonce";

        let signature = sign(&keypair.secret, data).unwrap();
        let valid = verify(&keypair.public, data, &signature).unwrap();

        assert!(valid);
    }

    #[test]
    fn verify_with_wrong_key_fails() {
        let keypair_a = generate_signing_keypair();
        let keypair_b = generate_signing_keypair();
        let data = b"some message";

        let signature = sign(&keypair_a.secret, data).unwrap();
        let valid = verify(&keypair_b.public, data, &signature).unwrap();

        assert!(!valid);
    }

    #[test]
    fn verify_with_tampered_data_fails() {
        let keypair = generate_signing_keypair();
        let data = b"original data";

        let signature = sign(&keypair.secret, data).unwrap();
        let valid = verify(&keypair.public, b"tampered data", &signature).unwrap();

        assert!(!valid);
    }

    #[test]
    fn verify_with_tampered_signature_fails() {
        let keypair = generate_signing_keypair();
        let data = b"some data";

        let mut signature = sign(&keypair.secret, data).unwrap();
        signature[0] ^= 0xff;

        let result = verify(&keypair.public, data, &signature);
        // Tampered signature may fail parse or verify — both are acceptable
        match result {
            Ok(valid) => assert!(!valid),
            Err(_) => {} // invalid signature format is also correct rejection
        }
    }

    #[test]
    fn sign_empty_data() {
        let keypair = generate_signing_keypair();
        let data = b"";

        let signature = sign(&keypair.secret, data).unwrap();
        let valid = verify(&keypair.public, data, &signature).unwrap();

        assert!(valid);
    }

    #[test]
    fn signature_is_64_bytes() {
        let keypair = generate_signing_keypair();
        let signature = sign(&keypair.secret, b"data").unwrap();

        assert_eq!(signature.len(), 64);
    }
}

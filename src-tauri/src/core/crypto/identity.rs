use blake2::digest::{consts::U32, Digest};
use blake2::Blake2b;
use ed25519_dalek::SigningKey;
use rand::rngs::OsRng;
use x25519_dalek::StaticSecret;

use crate::types::{CoreError, ExchangeKeypair, Identity, PeerId, SigningKeypair};

pub fn generate_identity(display_name: String) -> Identity {
    let signing_keypair = generate_signing_keypair();
    let exchange_keypair = generate_exchange_keypair();
    let peer_id = derive_peer_id(&signing_keypair.public);

    let created_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs();

    Identity {
        peer_id,
        signing_keypair,
        exchange_keypair,
        display_name,
        created_at,
    }
}

pub fn generate_signing_keypair() -> SigningKeypair {
    let signing_key = SigningKey::generate(&mut OsRng);
    let verifying_key = signing_key.verifying_key();

    SigningKeypair {
        secret: signing_key.to_keypair_bytes(),
        public: verifying_key.to_bytes(),
    }
}

pub fn generate_exchange_keypair() -> ExchangeKeypair {
    let secret = StaticSecret::random_from_rng(OsRng);
    let public = x25519_dalek::PublicKey::from(&secret);

    ExchangeKeypair {
        secret: secret.to_bytes(),
        public: public.to_bytes(),
    }
}

pub fn derive_peer_id(signing_public_key: &[u8; 32]) -> PeerId {
    let mut hasher = Blake2b::<U32>::new();
    hasher.update(signing_public_key);
    let hash = hasher.finalize();

    let mut peer_id = [0u8; 16];
    peer_id.copy_from_slice(&hash[..16]);
    peer_id
}

pub fn restore_signing_keypair(
    keypair_bytes: &[u8; 64],
) -> Result<SigningKeypair, CoreError> {
    let signing_key = SigningKey::from_keypair_bytes(keypair_bytes)
        .map_err(|e| CoreError::Crypto(format!("invalid signing keypair: {e}")))?;
    let verifying_key = signing_key.verifying_key();

    Ok(SigningKeypair {
        secret: signing_key.to_keypair_bytes(),
        public: verifying_key.to_bytes(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_identity_produces_valid_keys() {
        let identity = generate_identity("alice".to_string());

        assert_eq!(identity.display_name, "alice");
        assert_eq!(identity.peer_id.len(), 16);
        assert_eq!(identity.signing_keypair.secret.len(), 64);
        assert_eq!(identity.signing_keypair.public.len(), 32);
        assert_eq!(identity.exchange_keypair.secret.len(), 32);
        assert_eq!(identity.exchange_keypair.public.len(), 32);
        assert!(identity.created_at > 0);
    }

    #[test]
    fn peer_id_derived_from_signing_public_key() {
        let keypair = generate_signing_keypair();
        let peer_id_1 = derive_peer_id(&keypair.public);
        let peer_id_2 = derive_peer_id(&keypair.public);

        assert_eq!(peer_id_1, peer_id_2, "same public key must produce same peer_id");
    }

    #[test]
    fn different_keys_produce_different_peer_ids() {
        let keypair_a = generate_signing_keypair();
        let keypair_b = generate_signing_keypair();
        let peer_id_a = derive_peer_id(&keypair_a.public);
        let peer_id_b = derive_peer_id(&keypair_b.public);

        assert_ne!(peer_id_a, peer_id_b);
    }

    #[test]
    fn two_identities_have_different_keys() {
        let identity_a = generate_identity("a".to_string());
        let identity_b = generate_identity("b".to_string());

        assert_ne!(identity_a.signing_keypair.public, identity_b.signing_keypair.public);
        assert_ne!(identity_a.exchange_keypair.public, identity_b.exchange_keypair.public);
        assert_ne!(identity_a.peer_id, identity_b.peer_id);
    }

    #[test]
    fn restore_signing_keypair_roundtrip() {
        let original = generate_signing_keypair();
        let restored = restore_signing_keypair(&original.secret).unwrap();

        assert_eq!(original.public, restored.public);
        assert_eq!(original.secret, restored.secret);
    }

    #[test]
    fn restore_signing_keypair_rejects_invalid_bytes() {
        let invalid = [0u8; 64];
        let result = restore_signing_keypair(&invalid);

        assert!(result.is_err());
    }
}

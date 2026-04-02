use blake2::digest::consts::U32;
use blake2::Blake2b;
use hkdf::SimpleHkdf;
use x25519_dalek::{PublicKey, StaticSecret};

use crate::types::CoreError;

const HKDF_SALT: &[u8] = b"ghostmesh-dh-v1";
const HKDF_INFO: &[u8] = b"ghostmesh-key-exchange";

pub fn derive_shared_secret(
    my_secret: &[u8; 32],
    their_public: &[u8; 32],
) -> Result<[u8; 32], CoreError> {
    let secret = StaticSecret::from(*my_secret);
    let public = PublicKey::from(*their_public);

    let shared_point = secret.diffie_hellman(&public);

    let hkdf = SimpleHkdf::<Blake2b<U32>>::new(Some(HKDF_SALT), shared_point.as_bytes());

    let mut output_key = [0u8; 32];
    hkdf.expand(HKDF_INFO, &mut output_key)
        .map_err(|e| CoreError::Crypto(format!("hkdf expand failed: {e}")))?;

    Ok(output_key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::identity::generate_exchange_keypair;

    #[test]
    fn dh_roundtrip_both_sides_derive_same_key() {
        let alice = generate_exchange_keypair();
        let bob = generate_exchange_keypair();

        let shared_alice =
            derive_shared_secret(&alice.secret, &bob.public).unwrap();
        let shared_bob =
            derive_shared_secret(&bob.secret, &alice.public).unwrap();

        assert_eq!(shared_alice, shared_bob);
    }

    #[test]
    fn different_peers_produce_different_shared_secrets() {
        let alice = generate_exchange_keypair();
        let bob = generate_exchange_keypair();
        let carol = generate_exchange_keypair();

        let shared_ab =
            derive_shared_secret(&alice.secret, &bob.public).unwrap();
        let shared_ac =
            derive_shared_secret(&alice.secret, &carol.public).unwrap();

        assert_ne!(shared_ab, shared_ac);
    }

    #[test]
    fn shared_secret_is_32_bytes() {
        let alice = generate_exchange_keypair();
        let bob = generate_exchange_keypair();

        let shared = derive_shared_secret(&alice.secret, &bob.public).unwrap();

        assert_eq!(shared.len(), 32);
    }

    #[test]
    fn shared_secret_is_deterministic() {
        let alice = generate_exchange_keypair();
        let bob = generate_exchange_keypair();

        let shared_1 = derive_shared_secret(&alice.secret, &bob.public).unwrap();
        let shared_2 = derive_shared_secret(&alice.secret, &bob.public).unwrap();

        assert_eq!(shared_1, shared_2);
    }
}

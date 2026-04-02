use snow::{Builder, HandshakeState, Keypair, TransportState};

use crate::types::CoreError;

const NOISE_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2b";

pub fn generate_noise_keypair() -> Result<Keypair, CoreError> {
    Builder::new(NOISE_PATTERN.parse().map_err(noise_error)?)
        .generate_keypair()
        .map_err(noise_error)
}

pub fn build_initiator(local_keypair: &Keypair) -> Result<HandshakeState, CoreError> {
    Builder::new(NOISE_PATTERN.parse().map_err(noise_error)?)
        .local_private_key(&local_keypair.private)
        .build_initiator()
        .map_err(noise_error)
}

pub fn build_responder(local_keypair: &Keypair) -> Result<HandshakeState, CoreError> {
    Builder::new(NOISE_PATTERN.parse().map_err(noise_error)?)
        .local_private_key(&local_keypair.private)
        .build_responder()
        .map_err(noise_error)
}

pub struct NoiseTransport {
    transport: TransportState,
}

impl NoiseTransport {
    pub fn from_handshake(state: HandshakeState) -> Result<Self, CoreError> {
        let transport = state.into_transport_mode().map_err(noise_error)?;
        Ok(Self { transport })
    }

    pub fn encrypt(&mut self, plaintext: &[u8]) -> Result<Vec<u8>, CoreError> {
        let mut buffer = vec![0u8; plaintext.len() + 64]; // poly1305 tag overhead
        let len = self
            .transport
            .write_message(plaintext, &mut buffer)
            .map_err(noise_error)?;
        buffer.truncate(len);
        Ok(buffer)
    }

    pub fn decrypt(&mut self, ciphertext: &[u8]) -> Result<Vec<u8>, CoreError> {
        let mut buffer = vec![0u8; ciphertext.len()];
        let len = self
            .transport
            .read_message(ciphertext, &mut buffer)
            .map_err(noise_error)?;
        buffer.truncate(len);
        Ok(buffer)
    }

    pub fn remote_public_key(&self) -> Option<Vec<u8>> {
        self.transport.get_remote_static().map(|k| k.to_vec())
    }
}

fn noise_error(e: snow::Error) -> CoreError {
    CoreError::Crypto(format!("noise error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX_NOISE_MESSAGE_LEN: usize = 65535;

    fn complete_handshake(
        initiator_keypair: &Keypair,
        responder_keypair: &Keypair,
    ) -> (NoiseTransport, NoiseTransport) {
        let mut initiator = build_initiator(initiator_keypair).unwrap();
        let mut responder = build_responder(responder_keypair).unwrap();

        let mut buf = vec![0u8; MAX_NOISE_MESSAGE_LEN];
        let mut read_buf = vec![0u8; MAX_NOISE_MESSAGE_LEN];

        // Message 1: initiator -> responder
        let len = initiator.write_message(&[], &mut buf).unwrap();
        responder.read_message(&buf[..len], &mut read_buf).unwrap();

        // Message 2: responder -> initiator
        let len = responder.write_message(&[], &mut buf).unwrap();
        initiator.read_message(&buf[..len], &mut read_buf).unwrap();

        // Message 3: initiator -> responder
        let len = initiator.write_message(&[], &mut buf).unwrap();
        responder.read_message(&buf[..len], &mut read_buf).unwrap();

        let init_transport = NoiseTransport::from_handshake(initiator).unwrap();
        let resp_transport = NoiseTransport::from_handshake(responder).unwrap();

        (init_transport, resp_transport)
    }

    #[test]
    fn handshake_completes_successfully() {
        let initiator_kp = generate_noise_keypair().unwrap();
        let responder_kp = generate_noise_keypair().unwrap();

        let (init_transport, resp_transport) =
            complete_handshake(&initiator_kp, &responder_kp);

        assert!(init_transport.remote_public_key().is_some());
        assert!(resp_transport.remote_public_key().is_some());
    }

    #[test]
    fn remote_keys_exchanged_correctly() {
        let initiator_kp = generate_noise_keypair().unwrap();
        let responder_kp = generate_noise_keypair().unwrap();

        let (init_transport, resp_transport) =
            complete_handshake(&initiator_kp, &responder_kp);

        assert_eq!(
            init_transport.remote_public_key().unwrap(),
            responder_kp.public
        );
        assert_eq!(
            resp_transport.remote_public_key().unwrap(),
            initiator_kp.public
        );
    }

    #[test]
    fn encrypted_message_exchange_after_handshake() {
        let initiator_kp = generate_noise_keypair().unwrap();
        let responder_kp = generate_noise_keypair().unwrap();

        let (mut init_transport, mut resp_transport) =
            complete_handshake(&initiator_kp, &responder_kp);

        // Initiator sends to responder
        let plaintext = b"hello from initiator";
        let ciphertext = init_transport.encrypt(plaintext).unwrap();
        let decrypted = resp_transport.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, plaintext);

        // Responder sends to initiator
        let reply = b"hello from responder";
        let ciphertext = resp_transport.encrypt(reply).unwrap();
        let decrypted = init_transport.decrypt(&ciphertext).unwrap();
        assert_eq!(decrypted, reply);
    }

    #[test]
    fn multiple_messages_exchange() {
        let initiator_kp = generate_noise_keypair().unwrap();
        let responder_kp = generate_noise_keypair().unwrap();

        let (mut init_transport, mut resp_transport) =
            complete_handshake(&initiator_kp, &responder_kp);

        for i in 0..10 {
            let msg = format!("message {i}");
            let ct = init_transport.encrypt(msg.as_bytes()).unwrap();
            let pt = resp_transport.decrypt(&ct).unwrap();
            assert_eq!(pt, msg.as_bytes());

            let reply = format!("reply {i}");
            let ct = resp_transport.encrypt(reply.as_bytes()).unwrap();
            let pt = init_transport.decrypt(&ct).unwrap();
            assert_eq!(pt, reply.as_bytes());
        }
    }

    #[test]
    fn decrypt_with_wrong_transport_fails() {
        let kp_a = generate_noise_keypair().unwrap();
        let kp_b = generate_noise_keypair().unwrap();
        let kp_c = generate_noise_keypair().unwrap();

        let (mut transport_ab, _transport_ba) = complete_handshake(&kp_a, &kp_b);
        let (_transport_ac, mut transport_ca) = complete_handshake(&kp_a, &kp_c);

        let ciphertext = transport_ab.encrypt(b"secret").unwrap();
        let result = transport_ca.decrypt(&ciphertext);

        assert!(result.is_err());
    }

    #[test]
    fn encrypt_empty_message() {
        let kp_a = generate_noise_keypair().unwrap();
        let kp_b = generate_noise_keypair().unwrap();

        let (mut init, mut resp) = complete_handshake(&kp_a, &kp_b);

        let ct = init.encrypt(b"").unwrap();
        let pt = resp.decrypt(&ct).unwrap();
        assert_eq!(pt, b"");
    }
}

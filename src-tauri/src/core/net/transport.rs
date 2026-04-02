use serde::{Deserialize, Serialize};
use snow::Keypair;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::crypto::noise::{build_initiator, build_responder, NoiseTransport};
use crate::types::{CoreError, PeerId, WireMessage};

use super::wire::{decode_wire_message, encode_wire_message, frame_message, read_frame_length};

const PROTOCOL_VERSION: u8 = 1;
const MAX_HANDSHAKE_MESSAGE_LEN: usize = 65535;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthHello {
    pub peer_id: PeerId,
    pub signing_pk: [u8; 32],
    pub protocol_version: u8,
}

pub struct SecureConnection {
    stream: TcpStream,
    noise: NoiseTransport,
    remote_peer_id: PeerId,
    remote_signing_pk: [u8; 32],
}

impl SecureConnection {
    pub async fn connect(
        address: &str,
        local_keypair: &Keypair,
        local_peer_id: &PeerId,
        local_signing_pk: &[u8; 32],
    ) -> Result<Self, CoreError> {
        let stream = TcpStream::connect(address)
            .await
            .map_err(|e| CoreError::Net(format!("TCP connect error: {e}")))?;

        Self::perform_initiator_handshake(
            stream,
            local_keypair,
            local_peer_id,
            local_signing_pk,
        )
        .await
    }

    pub async fn accept(
        stream: TcpStream,
        local_keypair: &Keypair,
        local_peer_id: &PeerId,
        local_signing_pk: &[u8; 32],
    ) -> Result<Self, CoreError> {
        Self::perform_responder_handshake(
            stream,
            local_keypair,
            local_peer_id,
            local_signing_pk,
        )
        .await
    }

    pub async fn send(&mut self, message: &WireMessage) -> Result<(), CoreError> {
        let cbor_payload = encode_wire_message(message)?;
        let ciphertext = self.noise.encrypt(&cbor_payload)?;
        let framed = frame_message(&ciphertext)?;
        self.stream
            .write_all(&framed)
            .await
            .map_err(|e| CoreError::Net(format!("TCP write error: {e}")))?;
        self.stream
            .flush()
            .await
            .map_err(|e| CoreError::Net(format!("TCP flush error: {e}")))?;
        Ok(())
    }

    pub async fn receive(&mut self) -> Result<WireMessage, CoreError> {
        let mut length_header = [0u8; 4];
        self.stream
            .read_exact(&mut length_header)
            .await
            .map_err(|e| CoreError::Net(format!("TCP read length error: {e}")))?;

        let frame_length = read_frame_length(&length_header)?;
        let mut ciphertext = vec![0u8; frame_length];
        self.stream
            .read_exact(&mut ciphertext)
            .await
            .map_err(|e| CoreError::Net(format!("TCP read payload error: {e}")))?;

        let plaintext = self.noise.decrypt(&ciphertext)?;
        decode_wire_message(&plaintext)
    }

    pub fn remote_peer_id(&self) -> &PeerId {
        &self.remote_peer_id
    }

    pub fn remote_signing_pk(&self) -> &[u8; 32] {
        &self.remote_signing_pk
    }

    async fn perform_initiator_handshake(
        mut stream: TcpStream,
        local_keypair: &Keypair,
        local_peer_id: &PeerId,
        local_signing_pk: &[u8; 32],
    ) -> Result<Self, CoreError> {
        let mut handshake = build_initiator(local_keypair)?;
        let mut send_buffer = vec![0u8; MAX_HANDSHAKE_MESSAGE_LEN];
        let mut recv_buffer = vec![0u8; MAX_HANDSHAKE_MESSAGE_LEN];

        // Message 1: I -> R: e
        let len = handshake
            .write_message(&[], &mut send_buffer)
            .map_err(noise_error)?;
        write_length_prefixed(&mut stream, &send_buffer[..len]).await?;

        // Message 2: R -> I: e, ee, s, es
        let message2 = read_length_prefixed(&mut stream).await?;
        handshake
            .read_message(&message2, &mut recv_buffer)
            .map_err(noise_error)?;

        // Message 3: I -> R: s, se
        let len = handshake
            .write_message(&[], &mut send_buffer)
            .map_err(noise_error)?;
        write_length_prefixed(&mut stream, &send_buffer[..len]).await?;

        let mut noise = NoiseTransport::from_handshake(handshake)?;

        // Message 4: I -> R: AuthHello (encrypted)
        let local_auth = AuthHello {
            peer_id: *local_peer_id,
            signing_pk: *local_signing_pk,
            protocol_version: PROTOCOL_VERSION,
        };
        let auth_bytes = encode_auth_hello(&local_auth)?;
        let encrypted_auth = noise.encrypt(&auth_bytes)?;
        let framed_auth = frame_message(&encrypted_auth)?;
        stream
            .write_all(&framed_auth)
            .await
            .map_err(|e| CoreError::Net(format!("TCP write auth error: {e}")))?;
        stream
            .flush()
            .await
            .map_err(|e| CoreError::Net(format!("TCP flush auth error: {e}")))?;

        // Message 5: R -> I: AuthHello (encrypted)
        let remote_auth = receive_auth_hello(&mut stream, &mut noise).await?;

        Ok(Self {
            stream,
            noise,
            remote_peer_id: remote_auth.peer_id,
            remote_signing_pk: remote_auth.signing_pk,
        })
    }

    async fn perform_responder_handshake(
        mut stream: TcpStream,
        local_keypair: &Keypair,
        local_peer_id: &PeerId,
        local_signing_pk: &[u8; 32],
    ) -> Result<Self, CoreError> {
        let mut handshake = build_responder(local_keypair)?;
        let mut send_buffer = vec![0u8; MAX_HANDSHAKE_MESSAGE_LEN];
        let mut recv_buffer = vec![0u8; MAX_HANDSHAKE_MESSAGE_LEN];

        // Message 1: I -> R: e
        let message1 = read_length_prefixed(&mut stream).await?;
        handshake
            .read_message(&message1, &mut recv_buffer)
            .map_err(noise_error)?;

        // Message 2: R -> I: e, ee, s, es
        let len = handshake
            .write_message(&[], &mut send_buffer)
            .map_err(noise_error)?;
        write_length_prefixed(&mut stream, &send_buffer[..len]).await?;

        // Message 3: I -> R: s, se
        let message3 = read_length_prefixed(&mut stream).await?;
        handshake
            .read_message(&message3, &mut recv_buffer)
            .map_err(noise_error)?;

        let mut noise = NoiseTransport::from_handshake(handshake)?;

        // Message 4: I -> R: AuthHello (encrypted)
        let remote_auth = receive_auth_hello(&mut stream, &mut noise).await?;

        // Message 5: R -> I: AuthHello (encrypted)
        let local_auth = AuthHello {
            peer_id: *local_peer_id,
            signing_pk: *local_signing_pk,
            protocol_version: PROTOCOL_VERSION,
        };
        let auth_bytes = encode_auth_hello(&local_auth)?;
        let encrypted_auth = noise.encrypt(&auth_bytes)?;
        let framed_auth = frame_message(&encrypted_auth)?;
        stream
            .write_all(&framed_auth)
            .await
            .map_err(|e| CoreError::Net(format!("TCP write auth error: {e}")))?;
        stream
            .flush()
            .await
            .map_err(|e| CoreError::Net(format!("TCP flush auth error: {e}")))?;

        Ok(Self {
            stream,
            noise,
            remote_peer_id: remote_auth.peer_id,
            remote_signing_pk: remote_auth.signing_pk,
        })
    }
}

fn encode_auth_hello(auth: &AuthHello) -> Result<Vec<u8>, CoreError> {
    let mut buffer = Vec::new();
    ciborium::into_writer(auth, &mut buffer)
        .map_err(|e| CoreError::Net(format!("AuthHello CBOR encode error: {e}")))?;
    Ok(buffer)
}

fn decode_auth_hello(data: &[u8]) -> Result<AuthHello, CoreError> {
    ciborium::from_reader(data)
        .map_err(|e| CoreError::Net(format!("AuthHello CBOR decode error: {e}")))
}

async fn receive_auth_hello(
    stream: &mut TcpStream,
    noise: &mut NoiseTransport,
) -> Result<AuthHello, CoreError> {
    let mut length_header = [0u8; 4];
    stream
        .read_exact(&mut length_header)
        .await
        .map_err(|e| CoreError::Net(format!("TCP read auth length error: {e}")))?;

    let frame_length = read_frame_length(&length_header)?;
    let mut encrypted_data = vec![0u8; frame_length];
    stream
        .read_exact(&mut encrypted_data)
        .await
        .map_err(|e| CoreError::Net(format!("TCP read auth payload error: {e}")))?;

    let decrypted = noise.decrypt(&encrypted_data)?;
    decode_auth_hello(&decrypted)
}

async fn write_length_prefixed(stream: &mut TcpStream, data: &[u8]) -> Result<(), CoreError> {
    let length = (data.len() as u16).to_be_bytes();
    stream
        .write_all(&length)
        .await
        .map_err(|e| CoreError::Net(format!("TCP write handshake length error: {e}")))?;
    stream
        .write_all(data)
        .await
        .map_err(|e| CoreError::Net(format!("TCP write handshake data error: {e}")))?;
    stream
        .flush()
        .await
        .map_err(|e| CoreError::Net(format!("TCP flush handshake error: {e}")))?;
    Ok(())
}

async fn read_length_prefixed(stream: &mut TcpStream) -> Result<Vec<u8>, CoreError> {
    let mut length_bytes = [0u8; 2];
    stream
        .read_exact(&mut length_bytes)
        .await
        .map_err(|e| CoreError::Net(format!("TCP read handshake length error: {e}")))?;

    let length = u16::from_be_bytes(length_bytes) as usize;
    if length > MAX_HANDSHAKE_MESSAGE_LEN {
        return Err(CoreError::Net(format!(
            "handshake message too large: {length} bytes"
        )));
    }

    let mut data = vec![0u8; length];
    stream
        .read_exact(&mut data)
        .await
        .map_err(|e| CoreError::Net(format!("TCP read handshake data error: {e}")))?;
    Ok(data)
}

fn noise_error(e: snow::Error) -> CoreError {
    CoreError::Net(format!("noise handshake error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::noise::generate_noise_keypair;
    use tokio::net::TcpListener;

    fn test_peer_id(byte: u8) -> PeerId {
        [byte; 16]
    }

    fn test_signing_pk(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    #[test]
    fn auth_hello_cbor_roundtrip() {
        let original = AuthHello {
            peer_id: test_peer_id(0xAA),
            signing_pk: test_signing_pk(0xBB),
            protocol_version: 1,
        };
        let encoded = encode_auth_hello(&original).unwrap();
        let decoded = decode_auth_hello(&encoded).unwrap();
        assert_eq!(decoded.peer_id, original.peer_id);
        assert_eq!(decoded.signing_pk, original.signing_pk);
        assert_eq!(decoded.protocol_version, original.protocol_version);
    }

    #[tokio::test]
    async fn handshake_and_message_exchange() {
        let initiator_noise_kp = generate_noise_keypair().unwrap();
        let responder_noise_kp = generate_noise_keypair().unwrap();

        let initiator_peer_id = test_peer_id(0x01);
        let responder_peer_id = test_peer_id(0x02);
        let initiator_signing_pk = test_signing_pk(0x11);
        let responder_signing_pk = test_signing_pk(0x22);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let listen_addr = listener.local_addr().unwrap();

        let responder_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            SecureConnection::accept(
                stream,
                &responder_noise_kp,
                &responder_peer_id,
                &responder_signing_pk,
            )
            .await
            .unwrap()
        });

        let mut initiator_conn = SecureConnection::connect(
            &listen_addr.to_string(),
            &initiator_noise_kp,
            &initiator_peer_id,
            &initiator_signing_pk,
        )
        .await
        .unwrap();

        let mut responder_conn = responder_handle.await.unwrap();

        // Verify peer identity exchange
        assert_eq!(*initiator_conn.remote_peer_id(), responder_peer_id);
        assert_eq!(*responder_conn.remote_peer_id(), initiator_peer_id);
        assert_eq!(*initiator_conn.remote_signing_pk(), responder_signing_pk);
        assert_eq!(*responder_conn.remote_signing_pk(), initiator_signing_pk);

        // Send WireMessage from initiator to responder
        let ping = WireMessage::Ping {
            timestamp: 12345,
        };
        initiator_conn.send(&ping).await.unwrap();

        let received = responder_conn.receive().await.unwrap();
        match received {
            WireMessage::Ping { timestamp } => assert_eq!(timestamp, 12345),
            other => panic!("expected Ping, got {other:?}"),
        }

        // Send WireMessage from responder to initiator
        let pong = WireMessage::Pong {
            timestamp: 12345,
        };
        responder_conn.send(&pong).await.unwrap();

        let received = initiator_conn.receive().await.unwrap();
        match received {
            WireMessage::Pong { timestamp } => assert_eq!(timestamp, 12345),
            other => panic!("expected Pong, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn multiple_messages_exchange() {
        let initiator_noise_kp = generate_noise_keypair().unwrap();
        let responder_noise_kp = generate_noise_keypair().unwrap();

        let initiator_peer_id = test_peer_id(0x03);
        let responder_peer_id = test_peer_id(0x04);
        let initiator_signing_pk = test_signing_pk(0x33);
        let responder_signing_pk = test_signing_pk(0x44);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let listen_addr = listener.local_addr().unwrap();

        let responder_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            SecureConnection::accept(
                stream,
                &responder_noise_kp,
                &responder_peer_id,
                &responder_signing_pk,
            )
            .await
            .unwrap()
        });

        let mut initiator_conn = SecureConnection::connect(
            &listen_addr.to_string(),
            &initiator_noise_kp,
            &initiator_peer_id,
            &initiator_signing_pk,
        )
        .await
        .unwrap();

        let mut responder_conn = responder_handle.await.unwrap();

        for i in 0..5u64 {
            let ping = WireMessage::Ping { timestamp: i };
            initiator_conn.send(&ping).await.unwrap();

            let received = responder_conn.receive().await.unwrap();
            match received {
                WireMessage::Ping { timestamp } => assert_eq!(timestamp, i),
                other => panic!("expected Ping, got {other:?}"),
            }

            let pong = WireMessage::Pong { timestamp: i + 100 };
            responder_conn.send(&pong).await.unwrap();

            let received = initiator_conn.receive().await.unwrap();
            match received {
                WireMessage::Pong { timestamp } => assert_eq!(timestamp, i + 100),
                other => panic!("expected Pong, got {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn complex_wire_message_through_secure_channel() {
        let initiator_noise_kp = generate_noise_keypair().unwrap();
        let responder_noise_kp = generate_noise_keypair().unwrap();

        let initiator_peer_id = test_peer_id(0x05);
        let responder_peer_id = test_peer_id(0x06);
        let initiator_signing_pk = test_signing_pk(0x55);
        let responder_signing_pk = test_signing_pk(0x66);

        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let listen_addr = listener.local_addr().unwrap();

        let responder_handle = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            SecureConnection::accept(
                stream,
                &responder_noise_kp,
                &responder_peer_id,
                &responder_signing_pk,
            )
            .await
            .unwrap()
        });

        let mut initiator_conn = SecureConnection::connect(
            &listen_addr.to_string(),
            &initiator_noise_kp,
            &initiator_peer_id,
            &initiator_signing_pk,
        )
        .await
        .unwrap();

        let mut responder_conn = responder_handle.await.unwrap();

        let sync_request = WireMessage::SyncRequest {
            chat_id: [0xFF; 16],
            frontier: vec![crate::types::FrontierEntry {
                author_peer_id: initiator_peer_id,
                max_lamport_ts: 42,
                message_count: 10,
            }],
        };
        initiator_conn.send(&sync_request).await.unwrap();

        let received = responder_conn.receive().await.unwrap();
        match received {
            WireMessage::SyncRequest { chat_id, frontier } => {
                assert_eq!(chat_id, [0xFF; 16]);
                assert_eq!(frontier.len(), 1);
                assert_eq!(frontier[0].max_lamport_ts, 42);
            }
            other => panic!("expected SyncRequest, got {other:?}"),
        }
    }
}

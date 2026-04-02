use crate::types::{CoreError, WireMessage};

pub const MAX_FRAME_SIZE: usize = 256 * 1024;

pub fn encode_wire_message(message: &WireMessage) -> Result<Vec<u8>, CoreError> {
    let mut buffer = Vec::new();
    ciborium::into_writer(message, &mut buffer)
        .map_err(|e| CoreError::Net(format!("CBOR encode error: {e}")))?;
    Ok(buffer)
}

pub fn decode_wire_message(data: &[u8]) -> Result<WireMessage, CoreError> {
    ciborium::from_reader(data)
        .map_err(|e| CoreError::Net(format!("CBOR decode error: {e}")))
}

pub fn frame_message(data: &[u8]) -> Result<Vec<u8>, CoreError> {
    if data.len() > MAX_FRAME_SIZE {
        return Err(CoreError::Net(format!(
            "frame payload too large: {} bytes, max {} bytes",
            data.len(),
            MAX_FRAME_SIZE
        )));
    }
    let length = data.len() as u32;
    let mut framed = Vec::with_capacity(4 + data.len());
    framed.extend_from_slice(&length.to_be_bytes());
    framed.extend_from_slice(data);
    Ok(framed)
}

pub fn read_frame_length(header: &[u8; 4]) -> Result<usize, CoreError> {
    let length = u32::from_be_bytes(*header) as usize;
    if length > MAX_FRAME_SIZE {
        return Err(CoreError::Net(format!(
            "frame length too large: {length} bytes, max {MAX_FRAME_SIZE} bytes"
        )));
    }
    Ok(length)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::*;

    fn sample_frontier() -> Vec<FrontierEntry> {
        vec![FrontierEntry {
            author_peer_id: [1u8; 16],
            max_lamport_ts: 42,
            message_count: 10,
        }]
    }

    fn sample_message() -> Message {
        Message {
            message_id: [0xAA; 32],
            chat_id: [0xBB; 16],
            author_peer_id: [0xCC; 16],
            lamport_ts: 5,
            created_at: 1700000000,
            key_epoch: 1,
            parent_ids: vec![[0xDD; 32]],
            signature: vec![0x01; 64],
            payload_ciphertext: vec![0x02; 128],
            payload_nonce: [0x03; 24],
            received_at: 1700000001,
        }
    }

    fn sample_peer_identity() -> PeerIdentityPacket {
        PeerIdentityPacket {
            peer_id: [0x11; 16],
            signing_pk: [0x22; 32],
            exchange_pk: [0x33; 32],
            display_name: "Alice".to_string(),
        }
    }

    fn sample_peer_address() -> PeerAddress {
        PeerAddress {
            peer_id: [0x44; 16],
            address_type: "tcp".to_string(),
            address: "192.168.1.1:9473".to_string(),
            last_seen: 1700000000,
            last_successful: Some(1700000000),
            fail_count: 0,
        }
    }

    #[test]
    fn roundtrip_sync_request() {
        let original = WireMessage::SyncRequest {
            chat_id: [0xAA; 16],
            frontier: sample_frontier(),
        };
        let encoded = encode_wire_message(&original).unwrap();
        let decoded = decode_wire_message(&encoded).unwrap();
        match decoded {
            WireMessage::SyncRequest { chat_id, frontier } => {
                assert_eq!(chat_id, [0xAA; 16]);
                assert_eq!(frontier.len(), 1);
                assert_eq!(frontier[0].max_lamport_ts, 42);
            }
            other => panic!("expected SyncRequest, got {other:?}"),
        }
    }

    #[test]
    fn roundtrip_sync_response() {
        let original = WireMessage::SyncResponse {
            chat_id: [0xBB; 16],
            messages: vec![sample_message()],
            frontier: sample_frontier(),
        };
        let encoded = encode_wire_message(&original).unwrap();
        let decoded = decode_wire_message(&encoded).unwrap();
        match decoded {
            WireMessage::SyncResponse {
                chat_id,
                messages,
                frontier,
            } => {
                assert_eq!(chat_id, [0xBB; 16]);
                assert_eq!(messages.len(), 1);
                assert_eq!(messages[0].lamport_ts, 5);
                assert_eq!(frontier.len(), 1);
            }
            other => panic!("expected SyncResponse, got {other:?}"),
        }
    }

    #[test]
    fn roundtrip_sync_ack() {
        let original = WireMessage::SyncAck {
            chat_id: [0xCC; 16],
            received: vec![[0xDD; 32], [0xEE; 32]],
        };
        let encoded = encode_wire_message(&original).unwrap();
        let decoded = decode_wire_message(&encoded).unwrap();
        match decoded {
            WireMessage::SyncAck { chat_id, received } => {
                assert_eq!(chat_id, [0xCC; 16]);
                assert_eq!(received.len(), 2);
                assert_eq!(received[0], [0xDD; 32]);
                assert_eq!(received[1], [0xEE; 32]);
            }
            other => panic!("expected SyncAck, got {other:?}"),
        }
    }

    #[test]
    fn roundtrip_join_request() {
        let original = WireMessage::JoinRequest {
            chat_id: [0xFF; 16],
            invite_token: [0x55; 32],
            identity: sample_peer_identity(),
        };
        let encoded = encode_wire_message(&original).unwrap();
        let decoded = decode_wire_message(&encoded).unwrap();
        match decoded {
            WireMessage::JoinRequest {
                chat_id,
                invite_token,
                identity,
            } => {
                assert_eq!(chat_id, [0xFF; 16]);
                assert_eq!(invite_token, [0x55; 32]);
                assert_eq!(identity.display_name, "Alice");
                assert_eq!(identity.peer_id, [0x11; 16]);
            }
            other => panic!("expected JoinRequest, got {other:?}"),
        }
    }

    #[test]
    fn roundtrip_join_response() {
        let original = WireMessage::JoinResponse {
            accepted: true,
            group_key_enc: Some(vec![0x99; 48]),
            members: vec![],
            recent_messages: vec![sample_message()],
        };
        let encoded = encode_wire_message(&original).unwrap();
        let decoded = decode_wire_message(&encoded).unwrap();
        match decoded {
            WireMessage::JoinResponse {
                accepted,
                group_key_enc,
                members,
                recent_messages,
            } => {
                assert!(accepted);
                assert_eq!(group_key_enc.unwrap().len(), 48);
                assert!(members.is_empty());
                assert_eq!(recent_messages.len(), 1);
            }
            other => panic!("expected JoinResponse, got {other:?}"),
        }
    }

    #[test]
    fn roundtrip_peer_exchange() {
        let original = WireMessage::PeerExchange {
            chat_id: [0x77; 16],
            peers: vec![sample_peer_address()],
        };
        let encoded = encode_wire_message(&original).unwrap();
        let decoded = decode_wire_message(&encoded).unwrap();
        match decoded {
            WireMessage::PeerExchange { chat_id, peers } => {
                assert_eq!(chat_id, [0x77; 16]);
                assert_eq!(peers.len(), 1);
                assert_eq!(peers[0].address, "192.168.1.1:9473");
            }
            other => panic!("expected PeerExchange, got {other:?}"),
        }
    }

    #[test]
    fn roundtrip_ping() {
        let original = WireMessage::Ping {
            timestamp: 1700000042,
        };
        let encoded = encode_wire_message(&original).unwrap();
        let decoded = decode_wire_message(&encoded).unwrap();
        match decoded {
            WireMessage::Ping { timestamp } => assert_eq!(timestamp, 1700000042),
            other => panic!("expected Ping, got {other:?}"),
        }
    }

    #[test]
    fn roundtrip_pong() {
        let original = WireMessage::Pong {
            timestamp: 1700000043,
        };
        let encoded = encode_wire_message(&original).unwrap();
        let decoded = decode_wire_message(&encoded).unwrap();
        match decoded {
            WireMessage::Pong { timestamp } => assert_eq!(timestamp, 1700000043),
            other => panic!("expected Pong, got {other:?}"),
        }
    }

    #[test]
    fn frame_and_read_length_roundtrip() {
        let payload = vec![0xAB; 100];
        let framed = frame_message(&payload).unwrap();

        assert_eq!(framed.len(), 4 + 100);

        let header: [u8; 4] = framed[..4].try_into().unwrap();
        let length = read_frame_length(&header).unwrap();
        assert_eq!(length, 100);
        assert_eq!(&framed[4..], &payload[..]);
    }

    #[test]
    fn frame_empty_payload() {
        let framed = frame_message(&[]).unwrap();
        assert_eq!(framed.len(), 4);

        let header: [u8; 4] = framed[..4].try_into().unwrap();
        let length = read_frame_length(&header).unwrap();
        assert_eq!(length, 0);
    }

    #[test]
    fn frame_max_size_accepted() {
        let payload = vec![0u8; MAX_FRAME_SIZE];
        let framed = frame_message(&payload).unwrap();
        assert_eq!(framed.len(), 4 + MAX_FRAME_SIZE);
    }

    #[test]
    fn frame_oversized_rejected() {
        let payload = vec![0u8; MAX_FRAME_SIZE + 1];
        let result = frame_message(&payload);
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("too large"));
    }

    #[test]
    fn read_frame_length_oversized_rejected() {
        let length_bytes = ((MAX_FRAME_SIZE as u32) + 1).to_be_bytes();
        let result = read_frame_length(&length_bytes);
        assert!(result.is_err());
        let error_message = result.unwrap_err().to_string();
        assert!(error_message.contains("too large"));
    }

    #[test]
    fn full_wire_encode_frame_decode_roundtrip() {
        let original = WireMessage::Ping {
            timestamp: 9999,
        };
        let encoded = encode_wire_message(&original).unwrap();
        let framed = frame_message(&encoded).unwrap();

        let header: [u8; 4] = framed[..4].try_into().unwrap();
        let length = read_frame_length(&header).unwrap();
        let decoded = decode_wire_message(&framed[4..4 + length]).unwrap();

        match decoded {
            WireMessage::Ping { timestamp } => assert_eq!(timestamp, 9999),
            other => panic!("expected Ping, got {other:?}"),
        }
    }
}

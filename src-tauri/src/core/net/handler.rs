use std::time::{SystemTime, UNIX_EPOCH};

use crate::crypto::encrypt::decrypt_key_storage;
use crate::store::Store;
use crate::sync::engine::SyncEngine;
use crate::sync::lamport::LamportClock;
use crate::types::{CoreError, PeerId, WireMessage};

use super::event_sink::NetEventSink;

pub fn dispatch(
    message: &WireMessage,
    remote_peer_id: &PeerId,
    store: &Store,
    lamport: &mut LamportClock,
    password: &str,
    event_sink: &dyn NetEventSink,
) -> Result<Option<WireMessage>, CoreError> {
    match message {
        WireMessage::SyncRequest { chat_id, frontier } => {
            dispatch_sync_request(store, chat_id, frontier, remote_peer_id, event_sink)
        }
        WireMessage::SyncResponse {
            chat_id,
            messages,
            frontier,
        } => dispatch_sync_response(
            store,
            chat_id,
            messages.clone(),
            frontier,
            lamport,
            event_sink,
        ),
        WireMessage::SyncAck { chat_id, received } => {
            dispatch_sync_ack(store, chat_id, received, remote_peer_id, event_sink)
        }
        WireMessage::JoinRequest {
            chat_id,
            invite_token: _,
            identity,
        } => dispatch_join_request(
            store,
            chat_id,
            &identity.peer_id,
            &identity.exchange_pk,
            password,
        ),
        WireMessage::JoinResponse {
            accepted,
            group_key_enc,
            members,
            recent_messages,
        } => dispatch_join_response(
            store,
            *accepted,
            group_key_enc,
            members,
            recent_messages,
            password,
            remote_peer_id,
            event_sink,
        ),
        WireMessage::PeerExchange { chat_id: _, peers } => {
            dispatch_peer_exchange(store, peers)
        }
        WireMessage::Ping { timestamp } => Ok(Some(WireMessage::Pong {
            timestamp: *timestamp,
        })),
        WireMessage::Pong { .. } => Ok(None),
    }
}

fn dispatch_sync_request(
    store: &Store,
    chat_id: &[u8; 16],
    frontier: &[crate::types::FrontierEntry],
    remote_peer_id: &PeerId,
    event_sink: &dyn NetEventSink,
) -> Result<Option<WireMessage>, CoreError> {
    let (response, _sent_ids) = SyncEngine::handle_sync_request(store, chat_id, frontier)?;

    let sent_count = match &response {
        WireMessage::SyncResponse { messages, .. } => messages.len() as u64,
        _ => 0,
    };

    if sent_count > 0 {
        event_sink.on_sync_progress(chat_id, remote_peer_id, sent_count, sent_count);
    }

    Ok(Some(response))
}

fn dispatch_sync_response(
    store: &Store,
    chat_id: &[u8; 16],
    messages: Vec<crate::types::Message>,
    frontier: &[crate::types::FrontierEntry],
    lamport: &mut LamportClock,
    event_sink: &dyn NetEventSink,
) -> Result<Option<WireMessage>, CoreError> {
    let new_message_count = messages.len() as u64;
    let ack = SyncEngine::handle_sync_response(store, chat_id, messages, frontier, lamport)?;

    if new_message_count > 0 {
        event_sink.on_sync_complete(chat_id, new_message_count);
    }

    Ok(Some(ack))
}

fn dispatch_sync_ack(
    store: &Store,
    chat_id: &[u8; 16],
    received: &[crate::types::MessageId],
    remote_peer_id: &PeerId,
    event_sink: &dyn NetEventSink,
) -> Result<Option<WireMessage>, CoreError> {
    SyncEngine::handle_sync_ack(store, chat_id, received, remote_peer_id)?;

    for message_id in received {
        event_sink.on_delivery_ack(message_id, remote_peer_id);
    }

    Ok(None)
}

fn dispatch_join_request(
    store: &Store,
    chat_id: &[u8; 16],
    joiner_peer_id: &PeerId,
    joiner_exchange_pk: &[u8; 32],
    owner_password: &str,
) -> Result<Option<WireMessage>, CoreError> {
    let identity = store
        .get_identity()
        .map_err(|e| CoreError::Net(format!("failed to load identity: {e}")))?
        .ok_or(CoreError::IdentityNotInitialized)?;

    let owner_exchange_sk_bytes =
        decrypt_key_storage(owner_password, &identity.exchange_sk_enc)?;

    let owner_exchange_sk: [u8; 32] = owner_exchange_sk_bytes
        .try_into()
        .map_err(|_| CoreError::Crypto("exchange secret key has wrong length".to_string()))?;

    let response = SyncEngine::handle_join_request(
        store,
        chat_id,
        joiner_peer_id,
        joiner_exchange_pk,
        &owner_exchange_sk,
        owner_password,
    )?;

    Ok(Some(response))
}

fn dispatch_join_response(
    store: &Store,
    accepted: bool,
    group_key_enc: &Option<Vec<u8>>,
    members: &[crate::types::ChatMember],
    recent_messages: &[crate::types::Message],
    joiner_password: &str,
    owner_peer_id: &PeerId,
    event_sink: &dyn NetEventSink,
) -> Result<Option<WireMessage>, CoreError> {
    if !accepted {
        tracing::warn!("join response rejected by owner");
        return Ok(None);
    }

    let sealed_group_key = group_key_enc
        .as_ref()
        .ok_or_else(|| CoreError::Net("accepted JoinResponse missing group key".to_string()))?;

    let identity = store
        .get_identity()
        .map_err(|e| CoreError::Net(format!("failed to load identity: {e}")))?
        .ok_or(CoreError::IdentityNotInitialized)?;

    let joiner_exchange_sk_bytes =
        decrypt_key_storage(joiner_password, &identity.exchange_sk_enc)?;

    let joiner_exchange_sk: [u8; 32] = joiner_exchange_sk_bytes
        .try_into()
        .map_err(|_| CoreError::Crypto("exchange secret key has wrong length".to_string()))?;

    let owner_member = members
        .iter()
        .find(|m| m.peer_id == *owner_peer_id)
        .ok_or_else(|| CoreError::Net("owner not found in member list".to_string()))?;

    let chat_id = owner_member.chat_id;

    SyncEngine::handle_join_response(
        store,
        &chat_id,
        sealed_group_key,
        members,
        recent_messages,
        &joiner_exchange_sk,
        &owner_member.exchange_pk,
        joiner_password,
    )?;

    let chat = store.get_chat(&chat_id)?;
    let chat_name = chat.map(|c| c.chat_name).unwrap_or_default();
    event_sink.on_chat_join_complete(&chat_id, &chat_name);

    Ok(None)
}

pub fn prepare_peer_exchange(
    store: &Store,
    chat_id: &[u8; 16],
) -> Result<Option<WireMessage>, CoreError> {
    let addresses = store.get_all_peer_addresses()?;
    if addresses.is_empty() {
        return Ok(None);
    }
    Ok(Some(WireMessage::PeerExchange {
        chat_id: *chat_id,
        peers: addresses,
    }))
}

fn dispatch_peer_exchange(
    store: &Store,
    peers: &[crate::types::PeerAddress],
) -> Result<Option<WireMessage>, CoreError> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| CoreError::Net("system clock before unix epoch".to_string()))?
        .as_secs();

    for peer_address in peers {
        let mut address = peer_address.clone();
        address.last_seen = now;
        store.upsert_peer_address(&address)?;
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use crate::types::{
        Chat, ChatId, ChatKey, ChatMember, FrontierEntry, MemberRole, Message, MessageId,
        OutboxEntry, PeerAddress, PeerId, PeerIdentityPacket, WireMessage,
    };

    use std::sync::Mutex;

    struct TestEventSink {
        sync_progress_calls: Mutex<Vec<(ChatId, PeerId, u64, u64)>>,
        sync_complete_calls: Mutex<Vec<(ChatId, u64)>>,
        delivery_ack_calls: Mutex<Vec<(MessageId, PeerId)>>,
        chat_join_complete_calls: Mutex<Vec<(ChatId, String)>>,
    }

    impl TestEventSink {
        fn new() -> Self {
            Self {
                sync_progress_calls: Mutex::new(Vec::new()),
                sync_complete_calls: Mutex::new(Vec::new()),
                delivery_ack_calls: Mutex::new(Vec::new()),
                chat_join_complete_calls: Mutex::new(Vec::new()),
            }
        }
    }

    impl NetEventSink for TestEventSink {
        fn on_peer_connected(&self, _peer_id: &PeerId, _display_name: &str) {}
        fn on_peer_disconnected(&self, _peer_id: &PeerId, _display_name: &str) {}

        fn on_sync_progress(
            &self,
            chat_id: &ChatId,
            peer_id: &PeerId,
            received: u64,
            total: u64,
        ) {
            self.sync_progress_calls
                .lock()
                .unwrap()
                .push((*chat_id, *peer_id, received, total));
        }

        fn on_sync_complete(&self, chat_id: &ChatId, new_messages: u64) {
            self.sync_complete_calls
                .lock()
                .unwrap()
                .push((*chat_id, new_messages));
        }

        fn on_delivery_ack(&self, message_id: &MessageId, peer_id: &PeerId) {
            self.delivery_ack_calls
                .lock()
                .unwrap()
                .push((*message_id, *peer_id));
        }

        fn on_network_status(&self, _connected_peers: u32, _outbox_size: u32) {}

        fn on_chat_join_complete(&self, chat_id: &ChatId, chat_name: &str) {
            self.chat_join_complete_calls
                .lock()
                .unwrap()
                .push((*chat_id, chat_name.to_string()));
        }
    }

    fn test_store() -> Store {
        Store::open_in_memory().unwrap()
    }

    fn chat_id() -> ChatId {
        [0x10; 16]
    }

    fn peer_a() -> PeerId {
        [0xAA; 16]
    }

    fn peer_b() -> PeerId {
        [0xBB; 16]
    }

    fn setup_chat(store: &Store) {
        store
            .insert_chat(&Chat {
                chat_id: chat_id(),
                chat_name: "test-chat".to_string(),
                owner_peer_id: peer_a(),
                created_at: 1000,
                my_lamport_counter: 0,
            })
            .unwrap();
    }

    fn make_message(author: PeerId, lamport_ts: u64, unique_byte: u8) -> Message {
        let mut message_id = [0u8; 32];
        message_id[0] = unique_byte;
        message_id[1] = lamport_ts as u8;
        message_id[2] = author[0];

        Message {
            message_id,
            chat_id: chat_id(),
            author_peer_id: author,
            lamport_ts,
            created_at: 1000 + lamport_ts,
            key_epoch: 0,
            parent_ids: Vec::new(),
            signature: vec![0xAA; 64],
            payload_ciphertext: vec![0xBB; 32],
            payload_nonce: [0xCC; 24],
            received_at: 2000 + lamport_ts,
        }
    }

    #[test]
    fn ping_returns_pong() {
        let store = test_store();
        let mut lamport = LamportClock::new();
        let sink = TestEventSink::new();

        let ping = WireMessage::Ping { timestamp: 42 };
        let result = dispatch(&ping, &peer_a(), &store, &mut lamport, "", &sink).unwrap();

        match result {
            Some(WireMessage::Pong { timestamp }) => assert_eq!(timestamp, 42),
            other => panic!("expected Pong, got {other:?}"),
        }
    }

    #[test]
    fn pong_returns_none() {
        let store = test_store();
        let mut lamport = LamportClock::new();
        let sink = TestEventSink::new();

        let pong = WireMessage::Pong { timestamp: 42 };
        let result = dispatch(&pong, &peer_a(), &store, &mut lamport, "", &sink).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn sync_request_returns_sync_response() {
        let store = test_store();
        setup_chat(&store);
        let mut lamport = LamportClock::new();
        let sink = TestEventSink::new();

        let msg = make_message(peer_a(), 1, 0x01);
        store.insert_message(&msg).unwrap();

        let request = WireMessage::SyncRequest {
            chat_id: chat_id(),
            frontier: vec![],
        };

        let result =
            dispatch(&request, &peer_b(), &store, &mut lamport, "", &sink).unwrap();

        match result {
            Some(WireMessage::SyncResponse {
                chat_id: cid,
                messages,
                frontier,
            }) => {
                assert_eq!(cid, chat_id());
                assert_eq!(messages.len(), 1);
                assert!(!frontier.is_empty());
            }
            other => panic!("expected SyncResponse, got {other:?}"),
        }

        let progress = sink.sync_progress_calls.lock().unwrap();
        assert_eq!(progress.len(), 1);
        assert_eq!(progress[0].0, chat_id());
    }

    #[test]
    fn sync_response_returns_sync_ack_and_emits_complete() {
        let store = test_store();
        setup_chat(&store);
        let mut lamport = LamportClock::new();
        let sink = TestEventSink::new();

        let msg = make_message(peer_b(), 5, 0x02);
        let frontier = vec![FrontierEntry {
            author_peer_id: peer_b(),
            max_lamport_ts: 5,
            message_count: 1,
        }];

        let response = WireMessage::SyncResponse {
            chat_id: chat_id(),
            messages: vec![msg.clone()],
            frontier,
        };

        let result =
            dispatch(&response, &peer_b(), &store, &mut lamport, "", &sink).unwrap();

        match result {
            Some(WireMessage::SyncAck { chat_id: cid, received }) => {
                assert_eq!(cid, chat_id());
                assert_eq!(received.len(), 1);
                assert_eq!(received[0], msg.message_id);
            }
            other => panic!("expected SyncAck, got {other:?}"),
        }

        assert!(lamport.current() >= 5);

        let complete = sink.sync_complete_calls.lock().unwrap();
        assert_eq!(complete.len(), 1);
        assert_eq!(complete[0], (chat_id(), 1));
    }

    #[test]
    fn sync_ack_removes_outbox_entries_and_emits_ack() {
        let store = test_store();
        setup_chat(&store);
        let mut lamport = LamportClock::new();
        let sink = TestEventSink::new();

        let msg = make_message(peer_a(), 1, 0x03);
        store.insert_message(&msg).unwrap();
        store
            .insert_outbox_entry(&OutboxEntry {
                message_id: msg.message_id,
                target_peer_id: peer_b(),
                chat_id: chat_id(),
                created_at: 1000,
            })
            .unwrap();

        let ack = WireMessage::SyncAck {
            chat_id: chat_id(),
            received: vec![msg.message_id],
        };

        let result =
            dispatch(&ack, &peer_b(), &store, &mut lamport, "", &sink).unwrap();
        assert!(result.is_none());

        let remaining = store.get_outbox_for_peer(&peer_b()).unwrap();
        assert!(remaining.is_empty());

        let acks = sink.delivery_ack_calls.lock().unwrap();
        assert_eq!(acks.len(), 1);
        assert_eq!(acks[0], (msg.message_id, peer_b()));
    }

    #[test]
    fn peer_exchange_stores_addresses() {
        let store = test_store();
        let mut lamport = LamportClock::new();
        let sink = TestEventSink::new();

        let exchange = WireMessage::PeerExchange {
            chat_id: chat_id(),
            peers: vec![PeerAddress {
                peer_id: peer_b(),
                address_type: "tcp".to_string(),
                address: "10.0.0.1:9473".to_string(),
                last_seen: 0,
                last_successful: None,
                fail_count: 0,
            }],
        };

        let result =
            dispatch(&exchange, &peer_a(), &store, &mut lamport, "", &sink).unwrap();
        assert!(result.is_none());

        let addresses = store.get_peer_addresses(&peer_b()).unwrap();
        assert_eq!(addresses.len(), 1);
        assert_eq!(addresses[0].address, "10.0.0.1:9473");
        assert!(addresses[0].last_seen > 0);
    }

    #[test]
    fn prepare_peer_exchange_returns_none_when_no_addresses() {
        let store = test_store();

        let result = prepare_peer_exchange(&store, &chat_id()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn prepare_peer_exchange_includes_all_addresses() {
        let store = test_store();

        store
            .upsert_peer_address(&PeerAddress {
                peer_id: peer_a(),
                address_type: "tcp".to_string(),
                address: "10.0.0.1:9473".to_string(),
                last_seen: 1000,
                last_successful: None,
                fail_count: 0,
            })
            .unwrap();
        store
            .upsert_peer_address(&PeerAddress {
                peer_id: peer_b(),
                address_type: "tcp".to_string(),
                address: "10.0.0.2:9473".to_string(),
                last_seen: 2000,
                last_successful: None,
                fail_count: 0,
            })
            .unwrap();

        let result = prepare_peer_exchange(&store, &chat_id()).unwrap();
        match result {
            Some(WireMessage::PeerExchange { peers, .. }) => {
                assert_eq!(peers.len(), 2);
            }
            other => panic!("expected PeerExchange with 2 peers, got {other:?}"),
        }
    }

    #[test]
    fn prepare_peer_exchange_returns_message_with_addresses() {
        let store = test_store();

        store
            .upsert_peer_address(&PeerAddress {
                peer_id: peer_b(),
                address_type: "tcp".to_string(),
                address: "10.0.0.1:9473".to_string(),
                last_seen: 1000,
                last_successful: None,
                fail_count: 0,
            })
            .unwrap();

        let result = prepare_peer_exchange(&store, &chat_id()).unwrap();
        match result {
            Some(WireMessage::PeerExchange { chat_id: cid, peers }) => {
                assert_eq!(cid, chat_id());
                assert_eq!(peers.len(), 1);
                assert_eq!(peers[0].peer_id, peer_b());
                assert_eq!(peers[0].address, "10.0.0.1:9473");
            }
            other => panic!("expected PeerExchange, got {other:?}"),
        }
    }
}

use crate::store::Store;
use crate::types::{ChatId, CoreError, FrontierEntry, Message, MessageId, PeerId, WireMessage};

use super::frontier::{compute_diff_messages, merge_frontiers};
use super::lamport::LamportClock;

pub struct SyncEngine;

impl SyncEngine {
    pub fn new() -> Self {
        SyncEngine
    }

    pub fn prepare_sync_request(
        store: &Store,
        chat_id: &ChatId,
    ) -> Result<WireMessage, CoreError> {
        let frontier = store.get_frontier(chat_id)?;
        Ok(WireMessage::SyncRequest {
            chat_id: *chat_id,
            frontier,
        })
    }

    pub fn handle_sync_request(
        store: &Store,
        chat_id: &ChatId,
        remote_frontier: &[FrontierEntry],
    ) -> Result<(WireMessage, Vec<MessageId>), CoreError> {
        let local_frontier = store.get_frontier(chat_id)?;
        let diff = compute_diff_messages(&local_frontier, remote_frontier, store, chat_id)?;

        let received_ids: Vec<MessageId> = diff.iter().map(|m| m.message_id).collect();

        let response = WireMessage::SyncResponse {
            chat_id: *chat_id,
            messages: diff,
            frontier: local_frontier,
        };

        Ok((response, received_ids))
    }

    pub fn handle_sync_response(
        store: &Store,
        chat_id: &ChatId,
        messages: Vec<Message>,
        remote_frontier: &[FrontierEntry],
        lamport: &mut LamportClock,
    ) -> Result<WireMessage, CoreError> {
        let mut received_ids = Vec::with_capacity(messages.len());

        for message in &messages {
            lamport.on_receive(message.lamport_ts);

            if store.get_message(&message.message_id)?.is_none() {
                store.insert_message(message)?;
            }
            received_ids.push(message.message_id);
        }

        let local_frontier = store.get_frontier(chat_id)?;
        let merged = merge_frontiers(&local_frontier, remote_frontier);

        let ack = WireMessage::SyncAck {
            chat_id: *chat_id,
            received: received_ids,
        };

        // Update frontier in store for any remote-only entries
        for entry in &merged {
            store.update_frontier(chat_id, &entry.author_peer_id, entry.max_lamport_ts)?;
        }

        Ok(ack)
    }

    pub fn handle_sync_ack(
        store: &Store,
        _chat_id: &ChatId,
        received_ids: &[MessageId],
        remote_peer_id: &PeerId,
    ) -> Result<(), CoreError> {
        for message_id in received_ids {
            store.delete_outbox_entry(message_id, remote_peer_id)?;
        }
        Ok(())
    }

    pub fn process_incoming_message(
        store: &Store,
        message: Message,
        lamport: &mut LamportClock,
    ) -> Result<(), CoreError> {
        lamport.on_receive(message.lamport_ts);

        if store.get_message(&message.message_id)?.is_none() {
            store.insert_message(&message)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use crate::types::{Chat, ChatId, Message, OutboxEntry, PeerId, WireMessage};

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
                chat_name: "engine-test".to_string(),
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

    // --- prepare_sync_request ---

    #[test]
    fn prepare_sync_request_empty_chat() {
        let store = test_store();
        setup_chat(&store);

        let wire = SyncEngine::prepare_sync_request(&store, &chat_id()).unwrap();
        match wire {
            WireMessage::SyncRequest { chat_id: cid, frontier } => {
                assert_eq!(cid, chat_id());
                assert!(frontier.is_empty());
            }
            _ => panic!("expected SyncRequest"),
        }
    }

    #[test]
    fn prepare_sync_request_with_messages() {
        let store = test_store();
        setup_chat(&store);

        store.insert_message(&make_message(peer_a(), 1, 0x01)).unwrap();
        store.insert_message(&make_message(peer_a(), 3, 0x02)).unwrap();

        let wire = SyncEngine::prepare_sync_request(&store, &chat_id()).unwrap();
        match wire {
            WireMessage::SyncRequest { frontier, .. } => {
                assert_eq!(frontier.len(), 1);
                assert_eq!(frontier[0].author_peer_id, peer_a());
                assert_eq!(frontier[0].max_lamport_ts, 3);
                assert_eq!(frontier[0].message_count, 2);
            }
            _ => panic!("expected SyncRequest"),
        }
    }

    // --- handle_sync_request ---

    #[test]
    fn handle_sync_request_sends_diff() {
        let store = test_store();
        setup_chat(&store);

        store.insert_message(&make_message(peer_a(), 1, 0x01)).unwrap();
        store.insert_message(&make_message(peer_a(), 3, 0x02)).unwrap();

        let remote_frontier = vec![FrontierEntry {
            author_peer_id: peer_a(),
            max_lamport_ts: 1,
            message_count: 1,
        }];

        let (wire, received_ids) =
            SyncEngine::handle_sync_request(&store, &chat_id(), &remote_frontier).unwrap();

        match wire {
            WireMessage::SyncResponse { messages, frontier, .. } => {
                assert_eq!(messages.len(), 1);
                assert_eq!(messages[0].lamport_ts, 3);
                assert_eq!(frontier.len(), 1);
                assert_eq!(frontier[0].max_lamport_ts, 3);
            }
            _ => panic!("expected SyncResponse"),
        }
        assert_eq!(received_ids.len(), 1);
    }

    #[test]
    fn handle_sync_request_empty_remote_sends_all() {
        let store = test_store();
        setup_chat(&store);

        store.insert_message(&make_message(peer_a(), 1, 0x01)).unwrap();
        store.insert_message(&make_message(peer_b(), 2, 0x02)).unwrap();

        let (wire, _) =
            SyncEngine::handle_sync_request(&store, &chat_id(), &[]).unwrap();

        match wire {
            WireMessage::SyncResponse { messages, .. } => {
                assert_eq!(messages.len(), 2);
            }
            _ => panic!("expected SyncResponse"),
        }
    }

    // --- handle_sync_response ---

    #[test]
    fn handle_sync_response_stores_messages_and_updates_lamport() {
        let store = test_store();
        setup_chat(&store);

        let mut lamport = LamportClock::new();

        let incoming_messages = vec![
            make_message(peer_b(), 5, 0x01),
            make_message(peer_b(), 10, 0x02),
        ];

        let remote_frontier = vec![FrontierEntry {
            author_peer_id: peer_b(),
            max_lamport_ts: 10,
            message_count: 2,
        }];

        let ack = SyncEngine::handle_sync_response(
            &store,
            &chat_id(),
            incoming_messages,
            &remote_frontier,
            &mut lamport,
        )
        .unwrap();

        // Lamport was updated: max(0, 5) + 1 = 6, then max(6, 10) + 1 = 11
        assert_eq!(lamport.current(), 11);

        // Messages stored
        assert!(store.get_message(&make_message(peer_b(), 5, 0x01).message_id).unwrap().is_some());
        assert!(store.get_message(&make_message(peer_b(), 10, 0x02).message_id).unwrap().is_some());

        // Ack contains received IDs
        match ack {
            WireMessage::SyncAck { received, .. } => {
                assert_eq!(received.len(), 2);
            }
            _ => panic!("expected SyncAck"),
        }
    }

    #[test]
    fn handle_sync_response_skips_duplicate_messages() {
        let store = test_store();
        setup_chat(&store);

        let msg = make_message(peer_a(), 1, 0x01);
        store.insert_message(&msg).unwrap();

        let mut lamport = LamportClock::with_value(5);

        let remote_frontier = vec![FrontierEntry {
            author_peer_id: peer_a(),
            max_lamport_ts: 1,
            message_count: 1,
        }];

        let ack = SyncEngine::handle_sync_response(
            &store,
            &chat_id(),
            vec![msg],
            &remote_frontier,
            &mut lamport,
        )
        .unwrap();

        // Lamport still updated even for duplicates
        assert_eq!(lamport.current(), 6);

        match ack {
            WireMessage::SyncAck { received, .. } => {
                assert_eq!(received.len(), 1);
            }
            _ => panic!("expected SyncAck"),
        }
    }

    // --- handle_sync_ack ---

    #[test]
    fn handle_sync_ack_removes_from_outbox() {
        let store = test_store();
        setup_chat(&store);

        let msg1_id = make_message(peer_a(), 1, 0x01).message_id;
        let msg2_id = make_message(peer_a(), 2, 0x02).message_id;

        store
            .insert_outbox_entry(&OutboxEntry {
                message_id: msg1_id,
                target_peer_id: peer_b(),
                chat_id: chat_id(),
                created_at: 1000,
            })
            .unwrap();
        store
            .insert_outbox_entry(&OutboxEntry {
                message_id: msg2_id,
                target_peer_id: peer_b(),
                chat_id: chat_id(),
                created_at: 1001,
            })
            .unwrap();

        SyncEngine::handle_sync_ack(&store, &chat_id(), &[msg1_id], &peer_b()).unwrap();

        let remaining = store.get_outbox_for_peer(&peer_b()).unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].message_id, msg2_id);
    }

    #[test]
    fn handle_sync_ack_with_no_matching_outbox() {
        let store = test_store();
        setup_chat(&store);

        let msg_id = [0xFF; 32];
        // Should not error even if no outbox entry exists
        SyncEngine::handle_sync_ack(&store, &chat_id(), &[msg_id], &peer_b()).unwrap();
    }

    // --- process_incoming_message ---

    #[test]
    fn process_incoming_message_stores_and_updates_lamport() {
        let store = test_store();
        setup_chat(&store);

        let mut lamport = LamportClock::with_value(3);
        let msg = make_message(peer_b(), 10, 0x01);

        SyncEngine::process_incoming_message(&store, msg.clone(), &mut lamport).unwrap();

        assert_eq!(lamport.current(), 11);
        assert!(store.get_message(&msg.message_id).unwrap().is_some());
    }

    #[test]
    fn process_incoming_message_skips_duplicate() {
        let store = test_store();
        setup_chat(&store);

        let msg = make_message(peer_b(), 5, 0x01);
        store.insert_message(&msg).unwrap();

        let mut lamport = LamportClock::new();
        SyncEngine::process_incoming_message(&store, msg, &mut lamport).unwrap();

        // Lamport still updated
        assert_eq!(lamport.current(), 6);

        // Still only one message in store
        let messages = store.get_messages(&chat_id(), None, 100).unwrap();
        assert_eq!(messages.len(), 1);
    }

    // --- Full sync cycle simulation ---

    #[test]
    fn full_sync_cycle_two_peers() {
        // Simulate: peer_a has messages 1,3; peer_b has messages 2,4
        // After sync, both should have all 4 messages

        let store_a = test_store();
        let store_b = test_store();

        // Both stores need the chat
        setup_chat(&store_a);
        setup_chat(&store_b);

        // Peer A inserts messages authored by peer_a
        let msg_a1 = make_message(peer_a(), 1, 0x01);
        let msg_a3 = make_message(peer_a(), 3, 0x03);
        store_a.insert_message(&msg_a1).unwrap();
        store_a.insert_message(&msg_a3).unwrap();

        // Peer B inserts messages authored by peer_b
        let msg_b2 = make_message(peer_b(), 2, 0x02);
        let msg_b4 = make_message(peer_b(), 4, 0x04);
        store_b.insert_message(&msg_b2).unwrap();
        store_b.insert_message(&msg_b4).unwrap();

        let mut lamport_a = LamportClock::with_value(3);
        let mut lamport_b = LamportClock::with_value(4);

        // Step 1: A sends SyncRequest to B
        let sync_request = SyncEngine::prepare_sync_request(&store_a, &chat_id()).unwrap();

        let remote_frontier_a = match &sync_request {
            WireMessage::SyncRequest { frontier, .. } => frontier.clone(),
            _ => panic!("expected SyncRequest"),
        };

        // Step 2: B handles SyncRequest, sends SyncResponse with its diff
        let (sync_response_b, _) =
            SyncEngine::handle_sync_request(&store_b, &chat_id(), &remote_frontier_a).unwrap();

        let (messages_from_b, frontier_b) = match &sync_response_b {
            WireMessage::SyncResponse {
                messages, frontier, ..
            } => (messages.clone(), frontier.clone()),
            _ => panic!("expected SyncResponse"),
        };

        // B should send msg_b2 and msg_b4 (unknown to A)
        assert_eq!(messages_from_b.len(), 2);

        // Step 3: A handles SyncResponse from B — stores B's messages, sends SyncAck
        let ack_from_a = SyncEngine::handle_sync_response(
            &store_a,
            &chat_id(),
            messages_from_b,
            &frontier_b,
            &mut lamport_a,
        )
        .unwrap();

        // A now has all 4 messages
        let all_messages_a = store_a.get_messages(&chat_id(), None, 100).unwrap();
        assert_eq!(all_messages_a.len(), 4);

        // Step 4: A also sends its diff to B (as SyncResponse)
        let (sync_response_a, _) =
            SyncEngine::handle_sync_request(&store_a, &chat_id(), &frontier_b).unwrap();

        let messages_from_a = match &sync_response_a {
            WireMessage::SyncResponse { messages, .. } => messages.clone(),
            _ => panic!("expected SyncResponse"),
        };

        // A should send msg_a1 and msg_a3 to B
        assert_eq!(messages_from_a.len(), 2);

        // B stores A's messages
        let frontier_a = store_a.get_frontier(&chat_id()).unwrap();
        let _ack_from_b = SyncEngine::handle_sync_response(
            &store_b,
            &chat_id(),
            messages_from_a,
            &frontier_a,
            &mut lamport_b,
        )
        .unwrap();

        // B now has all 4 messages
        let all_messages_b = store_b.get_messages(&chat_id(), None, 100).unwrap();
        assert_eq!(all_messages_b.len(), 4);

        // Verify ordering: (lamport_ts, author_peer_id)
        assert_eq!(all_messages_a[0].lamport_ts, 1);
        assert_eq!(all_messages_a[1].lamport_ts, 2);
        assert_eq!(all_messages_a[2].lamport_ts, 3);
        assert_eq!(all_messages_a[3].lamport_ts, 4);

        // Both Lamport clocks advanced
        assert!(lamport_a.current() > 3);
        assert!(lamport_b.current() > 4);

        // SyncAck
        let received_ids = match ack_from_a {
            WireMessage::SyncAck { received, .. } => received,
            _ => panic!("expected SyncAck"),
        };
        assert_eq!(received_ids.len(), 2);
    }

    #[test]
    fn sync_with_empty_peer() {
        let store_a = test_store();
        let store_b = test_store();

        setup_chat(&store_a);
        setup_chat(&store_b);

        // A has 3 messages, B has none
        store_a.insert_message(&make_message(peer_a(), 1, 0x01)).unwrap();
        store_a.insert_message(&make_message(peer_a(), 2, 0x02)).unwrap();
        store_a.insert_message(&make_message(peer_a(), 3, 0x03)).unwrap();

        let mut lamport_b = LamportClock::new();

        // A → SyncRequest
        let sync_request = SyncEngine::prepare_sync_request(&store_a, &chat_id()).unwrap();
        let frontier_a = match &sync_request {
            WireMessage::SyncRequest { frontier, .. } => frontier.clone(),
            _ => panic!("expected SyncRequest"),
        };

        // B handles request → sends empty response (no diff from B's side)
        let (response_b, _) =
            SyncEngine::handle_sync_request(&store_b, &chat_id(), &frontier_a).unwrap();

        let (messages_from_b, frontier_b) = match &response_b {
            WireMessage::SyncResponse { messages, frontier, .. } => {
                (messages.clone(), frontier.clone())
            }
            _ => panic!("expected SyncResponse"),
        };

        // B has nothing to send
        assert!(messages_from_b.is_empty());
        assert!(frontier_b.is_empty());

        // A sends its diff to B
        let (response_a, _) =
            SyncEngine::handle_sync_request(&store_a, &chat_id(), &frontier_b).unwrap();

        let messages_from_a = match &response_a {
            WireMessage::SyncResponse { messages, .. } => messages.clone(),
            _ => panic!("expected SyncResponse"),
        };

        assert_eq!(messages_from_a.len(), 3);

        // B receives all
        let frontier_a_updated = store_a.get_frontier(&chat_id()).unwrap();
        SyncEngine::handle_sync_response(
            &store_b,
            &chat_id(),
            messages_from_a,
            &frontier_a_updated,
            &mut lamport_b,
        )
        .unwrap();

        let all_messages_b = store_b.get_messages(&chat_id(), None, 100).unwrap();
        assert_eq!(all_messages_b.len(), 3);
        assert_eq!(lamport_b.current(), 4); // max(0,1)+1=2, max(2,2)+1=3, max(3,3)+1=4
    }

    #[test]
    fn sync_already_synchronized_is_noop() {
        let store_a = test_store();
        let store_b = test_store();

        setup_chat(&store_a);
        setup_chat(&store_b);

        let msg = make_message(peer_a(), 1, 0x01);
        store_a.insert_message(&msg).unwrap();
        store_b.insert_message(&msg).unwrap();

        let sync_req = SyncEngine::prepare_sync_request(&store_a, &chat_id()).unwrap();
        let frontier_a = match &sync_req {
            WireMessage::SyncRequest { frontier, .. } => frontier.clone(),
            _ => panic!("expected SyncRequest"),
        };

        let (response, _) =
            SyncEngine::handle_sync_request(&store_b, &chat_id(), &frontier_a).unwrap();

        match response {
            WireMessage::SyncResponse { messages, .. } => {
                assert!(messages.is_empty());
            }
            _ => panic!("expected SyncResponse"),
        }
    }
}

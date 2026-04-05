use crate::store::Store;
use crate::types::{ChatId, CoreError, FrontierEntry, Message, PeerId};

pub fn frontier_contains<'a>(
    frontier: &'a [FrontierEntry],
    peer_id: &PeerId,
) -> Option<&'a FrontierEntry> {
    frontier.iter().find(|entry| entry.author_peer_id == *peer_id)
}

pub fn merge_frontiers(
    local: &[FrontierEntry],
    remote: &[FrontierEntry],
) -> Vec<FrontierEntry> {
    let mut merged: Vec<FrontierEntry> = Vec::new();

    for local_entry in local {
        match frontier_contains(remote, &local_entry.author_peer_id) {
            Some(remote_entry) => {
                merged.push(FrontierEntry {
                    author_peer_id: local_entry.author_peer_id,
                    max_lamport_ts: local_entry.max_lamport_ts.max(remote_entry.max_lamport_ts),
                    message_count: local_entry.message_count.max(remote_entry.message_count),
                });
            }
            None => {
                merged.push(local_entry.clone());
            }
        }
    }

    for remote_entry in remote {
        if frontier_contains(local, &remote_entry.author_peer_id).is_none() {
            merged.push(remote_entry.clone());
        }
    }

    merged
}

pub fn compute_diff_messages(
    local_frontier: &[FrontierEntry],
    remote_frontier: &[FrontierEntry],
    store: &Store,
    chat_id: &ChatId,
) -> Result<Vec<Message>, CoreError> {
    let mut diff_messages = Vec::new();

    for local_entry in local_frontier {
        match frontier_contains(remote_frontier, &local_entry.author_peer_id) {
            None => {
                let messages =
                    store.get_messages_by_author(chat_id, &local_entry.author_peer_id)?;
                diff_messages.extend(messages);
            }
            Some(remote_entry) => {
                if local_entry.max_lamport_ts > remote_entry.max_lamport_ts {
                    let messages = store.get_messages_by_author_after(
                        chat_id,
                        &local_entry.author_peer_id,
                        remote_entry.max_lamport_ts,
                    )?;
                    diff_messages.extend(messages);
                }
            }
        }
    }

    Ok(diff_messages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use crate::types::{Chat, ChatId, FrontierEntry, Message, PeerId};

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

    fn peer_c() -> PeerId {
        [0xCC; 16]
    }

    fn setup_chat(store: &Store) {
        store
            .insert_chat(&Chat {
                chat_id: chat_id(),
                chat_name: "sync-test".to_string(),
                owner_peer_id: peer_a(),
                created_at: 1000,
                my_lamport_counter: 0,
                unread_count: 0,
            })
            .unwrap();
    }

    fn make_message(author: PeerId, lamport_ts: u64, unique_byte: u8) -> Message {
        let mut message_id = [0u8; 32];
        message_id[0] = unique_byte;
        message_id[1] = lamport_ts as u8;

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

    // --- frontier_contains ---

    #[test]
    fn frontier_contains_finds_existing_entry() {
        let frontier = vec![
            FrontierEntry {
                author_peer_id: peer_a(),
                max_lamport_ts: 5,
                message_count: 3,
            },
            FrontierEntry {
                author_peer_id: peer_b(),
                max_lamport_ts: 10,
                message_count: 7,
            },
        ];

        let found = frontier_contains(&frontier, &peer_b());
        assert!(found.is_some());
        assert_eq!(found.unwrap().max_lamport_ts, 10);
    }

    #[test]
    fn frontier_contains_returns_none_for_missing() {
        let frontier = vec![FrontierEntry {
            author_peer_id: peer_a(),
            max_lamport_ts: 5,
            message_count: 3,
        }];

        assert!(frontier_contains(&frontier, &peer_c()).is_none());
    }

    #[test]
    fn frontier_contains_empty_frontier() {
        assert!(frontier_contains(&[], &peer_a()).is_none());
    }

    // --- merge_frontiers ---

    #[test]
    fn merge_disjoint_frontiers() {
        let local = vec![FrontierEntry {
            author_peer_id: peer_a(),
            max_lamport_ts: 5,
            message_count: 3,
        }];
        let remote = vec![FrontierEntry {
            author_peer_id: peer_b(),
            max_lamport_ts: 10,
            message_count: 7,
        }];

        let merged = merge_frontiers(&local, &remote);
        assert_eq!(merged.len(), 2);

        let entry_a = frontier_contains(&merged, &peer_a()).unwrap();
        assert_eq!(entry_a.max_lamport_ts, 5);
        assert_eq!(entry_a.message_count, 3);

        let entry_b = frontier_contains(&merged, &peer_b()).unwrap();
        assert_eq!(entry_b.max_lamport_ts, 10);
        assert_eq!(entry_b.message_count, 7);
    }

    #[test]
    fn merge_overlapping_takes_max() {
        let local = vec![FrontierEntry {
            author_peer_id: peer_a(),
            max_lamport_ts: 5,
            message_count: 3,
        }];
        let remote = vec![FrontierEntry {
            author_peer_id: peer_a(),
            max_lamport_ts: 10,
            message_count: 7,
        }];

        let merged = merge_frontiers(&local, &remote);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].max_lamport_ts, 10);
        assert_eq!(merged[0].message_count, 7);
    }

    #[test]
    fn merge_overlapping_local_higher() {
        let local = vec![FrontierEntry {
            author_peer_id: peer_a(),
            max_lamport_ts: 15,
            message_count: 10,
        }];
        let remote = vec![FrontierEntry {
            author_peer_id: peer_a(),
            max_lamport_ts: 5,
            message_count: 3,
        }];

        let merged = merge_frontiers(&local, &remote);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].max_lamport_ts, 15);
        assert_eq!(merged[0].message_count, 10);
    }

    #[test]
    fn merge_empty_frontiers() {
        let merged = merge_frontiers(&[], &[]);
        assert!(merged.is_empty());
    }

    #[test]
    fn merge_one_empty() {
        let local = vec![FrontierEntry {
            author_peer_id: peer_a(),
            max_lamport_ts: 5,
            message_count: 3,
        }];

        let merged = merge_frontiers(&local, &[]);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].author_peer_id, peer_a());

        let merged_reversed = merge_frontiers(&[], &local);
        assert_eq!(merged_reversed.len(), 1);
        assert_eq!(merged_reversed[0].author_peer_id, peer_a());
    }

    // --- compute_diff_messages ---

    #[test]
    fn diff_with_empty_remote_sends_all() {
        let store = test_store();
        setup_chat(&store);

        store
            .insert_message(&make_message(peer_a(), 1, 0x01))
            .unwrap();
        store
            .insert_message(&make_message(peer_a(), 2, 0x02))
            .unwrap();

        let local_frontier = store.get_frontier(&chat_id()).unwrap();
        let remote_frontier: Vec<FrontierEntry> = Vec::new();

        let diff = compute_diff_messages(&local_frontier, &remote_frontier, &store, &chat_id())
            .unwrap();
        assert_eq!(diff.len(), 2);
    }

    #[test]
    fn diff_with_full_overlap_sends_nothing() {
        let store = test_store();
        setup_chat(&store);

        store
            .insert_message(&make_message(peer_a(), 1, 0x01))
            .unwrap();
        store
            .insert_message(&make_message(peer_a(), 2, 0x02))
            .unwrap();

        let frontier = store.get_frontier(&chat_id()).unwrap();

        let diff =
            compute_diff_messages(&frontier, &frontier, &store, &chat_id()).unwrap();
        assert!(diff.is_empty());
    }

    #[test]
    fn diff_with_partial_overlap_sends_newer() {
        let store = test_store();
        setup_chat(&store);

        store
            .insert_message(&make_message(peer_a(), 1, 0x01))
            .unwrap();
        store
            .insert_message(&make_message(peer_a(), 2, 0x02))
            .unwrap();
        store
            .insert_message(&make_message(peer_a(), 5, 0x03))
            .unwrap();

        let local_frontier = store.get_frontier(&chat_id()).unwrap();

        let remote_frontier = vec![FrontierEntry {
            author_peer_id: peer_a(),
            max_lamport_ts: 2,
            message_count: 2,
        }];

        let diff = compute_diff_messages(&local_frontier, &remote_frontier, &store, &chat_id())
            .unwrap();
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].lamport_ts, 5);
    }

    #[test]
    fn diff_with_unknown_author_sends_all_their_messages() {
        let store = test_store();
        setup_chat(&store);

        store
            .insert_message(&make_message(peer_a(), 1, 0x01))
            .unwrap();
        store
            .insert_message(&make_message(peer_b(), 3, 0x02))
            .unwrap();

        let local_frontier = store.get_frontier(&chat_id()).unwrap();

        let remote_frontier = vec![FrontierEntry {
            author_peer_id: peer_a(),
            max_lamport_ts: 1,
            message_count: 1,
        }];

        let diff = compute_diff_messages(&local_frontier, &remote_frontier, &store, &chat_id())
            .unwrap();
        assert_eq!(diff.len(), 1);
        assert_eq!(diff[0].author_peer_id, peer_b());
    }

    #[test]
    fn diff_empty_local_frontier_sends_nothing() {
        let store = test_store();
        setup_chat(&store);

        let remote_frontier = vec![FrontierEntry {
            author_peer_id: peer_a(),
            max_lamport_ts: 10,
            message_count: 5,
        }];

        let diff = compute_diff_messages(&[], &remote_frontier, &store, &chat_id()).unwrap();
        assert!(diff.is_empty());
    }

    #[test]
    fn diff_multiple_authors_mixed() {
        let store = test_store();
        setup_chat(&store);

        store
            .insert_message(&make_message(peer_a(), 1, 0x01))
            .unwrap();
        store
            .insert_message(&make_message(peer_a(), 3, 0x02))
            .unwrap();
        store
            .insert_message(&make_message(peer_b(), 2, 0x03))
            .unwrap();
        store
            .insert_message(&make_message(peer_b(), 4, 0x04))
            .unwrap();
        store
            .insert_message(&make_message(peer_c(), 5, 0x05))
            .unwrap();

        let local_frontier = store.get_frontier(&chat_id()).unwrap();

        let remote_frontier = vec![
            FrontierEntry {
                author_peer_id: peer_a(),
                max_lamport_ts: 3,
                message_count: 2,
            },
            FrontierEntry {
                author_peer_id: peer_b(),
                max_lamport_ts: 2,
                message_count: 1,
            },
            // peer_c not known to remote
        ];

        let diff = compute_diff_messages(&local_frontier, &remote_frontier, &store, &chat_id())
            .unwrap();

        // peer_a: up to date (local max 3 == remote max 3) → nothing
        // peer_b: local has ts=4 > remote max 2 → send ts=4
        // peer_c: unknown to remote → send ts=5
        assert_eq!(diff.len(), 2);

        let lamport_timestamps: Vec<u64> = diff.iter().map(|m| m.lamport_ts).collect();
        assert!(lamport_timestamps.contains(&4));
        assert!(lamport_timestamps.contains(&5));
    }
}

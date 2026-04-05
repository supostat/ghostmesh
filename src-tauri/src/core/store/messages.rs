use crate::types::{ChatId, CoreError, FrontierEntry, Message, MessageId, PeerId};

use super::db::Store;

impl Store {
    pub fn insert_message(&self, message: &Message) -> Result<(), CoreError> {
        let parent_ids_cbor = encode_parent_ids(&message.parent_ids)?;

        self.connection()
            .execute(
                "INSERT INTO messages
                 (message_id, chat_id, author_peer_id, lamport_ts, created_at,
                  key_epoch, parent_ids, signature, payload_ciphertext, payload_nonce, received_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                rusqlite::params![
                    message.message_id.as_slice(),
                    message.chat_id.as_slice(),
                    message.author_peer_id.as_slice(),
                    message.lamport_ts,
                    message.created_at,
                    message.key_epoch,
                    parent_ids_cbor,
                    message.signature,
                    message.payload_ciphertext,
                    message.payload_nonce.as_slice(),
                    message.received_at,
                ],
            )
            .map_err(|e| CoreError::Store(format!("failed to insert message: {e}")))?;

        self.update_frontier(&message.chat_id, &message.author_peer_id, message.lamport_ts)?;

        Ok(())
    }

    pub fn get_messages(
        &self,
        chat_id: &ChatId,
        before_lamport: Option<u64>,
        limit: u32,
    ) -> Result<Vec<Message>, CoreError> {
        let (sql, params): (&str, Vec<Box<dyn rusqlite::types::ToSql>>) = match before_lamport {
            Some(before) => (
                "SELECT message_id, chat_id, author_peer_id, lamport_ts, created_at,
                        key_epoch, parent_ids, signature, payload_ciphertext, payload_nonce, received_at
                 FROM messages
                 WHERE chat_id = ?1 AND lamport_ts < ?2
                 ORDER BY lamport_ts ASC, author_peer_id ASC
                 LIMIT ?3",
                vec![
                    Box::new(chat_id.to_vec()) as Box<dyn rusqlite::types::ToSql>,
                    Box::new(before as i64),
                    Box::new(limit as i64),
                ],
            ),
            None => (
                "SELECT message_id, chat_id, author_peer_id, lamport_ts, created_at,
                        key_epoch, parent_ids, signature, payload_ciphertext, payload_nonce, received_at
                 FROM messages
                 WHERE chat_id = ?1
                 ORDER BY lamport_ts ASC, author_peer_id ASC
                 LIMIT ?2",
                vec![
                    Box::new(chat_id.to_vec()) as Box<dyn rusqlite::types::ToSql>,
                    Box::new(limit as i64),
                ],
            ),
        };

        let mut statement = self
            .connection()
            .prepare(sql)
            .map_err(|e| CoreError::Store(format!("failed to prepare get_messages: {e}")))?;

        let params_refs: Vec<&dyn rusqlite::types::ToSql> =
            params.iter().map(|p| p.as_ref()).collect();

        let rows = statement
            .query_map(params_refs.as_slice(), |row| {
                let parent_ids_blob: Option<Vec<u8>> = row.get(6)?;
                Ok(MessageRow {
                    message_id: row.get::<_, Vec<u8>>(0)?,
                    chat_id: row.get::<_, Vec<u8>>(1)?,
                    author_peer_id: row.get::<_, Vec<u8>>(2)?,
                    lamport_ts: row.get(3)?,
                    created_at: row.get(4)?,
                    key_epoch: row.get(5)?,
                    parent_ids_blob,
                    signature: row.get(7)?,
                    payload_ciphertext: row.get(8)?,
                    payload_nonce: row.get::<_, Vec<u8>>(9)?,
                    received_at: row.get(10)?,
                })
            })
            .map_err(|e| CoreError::Store(format!("failed to query messages: {e}")))?;

        let mut messages = Vec::new();
        for row in rows {
            let raw = row
                .map_err(|e| CoreError::Store(format!("failed to read message row: {e}")))?;
            messages.push(message_row_to_message(raw)?);
        }
        Ok(messages)
    }

    pub fn get_message(&self, message_id: &MessageId) -> Result<Option<Message>, CoreError> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT message_id, chat_id, author_peer_id, lamport_ts, created_at,
                        key_epoch, parent_ids, signature, payload_ciphertext, payload_nonce, received_at
                 FROM messages WHERE message_id = ?1",
            )
            .map_err(|e| CoreError::Store(format!("failed to prepare get_message: {e}")))?;

        let mut rows = statement
            .query_map([message_id.as_slice()], |row| {
                let parent_ids_blob: Option<Vec<u8>> = row.get(6)?;
                Ok(MessageRow {
                    message_id: row.get::<_, Vec<u8>>(0)?,
                    chat_id: row.get::<_, Vec<u8>>(1)?,
                    author_peer_id: row.get::<_, Vec<u8>>(2)?,
                    lamport_ts: row.get(3)?,
                    created_at: row.get(4)?,
                    key_epoch: row.get(5)?,
                    parent_ids_blob,
                    signature: row.get(7)?,
                    payload_ciphertext: row.get(8)?,
                    payload_nonce: row.get::<_, Vec<u8>>(9)?,
                    received_at: row.get(10)?,
                })
            })
            .map_err(|e| CoreError::Store(format!("failed to query message: {e}")))?;

        match rows.next() {
            Some(row) => {
                let raw = row
                    .map_err(|e| CoreError::Store(format!("failed to read message row: {e}")))?;
                Ok(Some(message_row_to_message(raw)?))
            }
            None => Ok(None),
        }
    }

    pub fn get_frontier(&self, chat_id: &ChatId) -> Result<Vec<FrontierEntry>, CoreError> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT author_peer_id, max_lamport_ts, message_count
                 FROM frontiers WHERE chat_id = ?1",
            )
            .map_err(|e| CoreError::Store(format!("failed to prepare get_frontier: {e}")))?;

        let rows = statement
            .query_map([chat_id.as_slice()], |row| {
                Ok(FrontierEntry {
                    author_peer_id: blob_to_peer_id(row.get::<_, Vec<u8>>(0)?),
                    max_lamport_ts: row.get(1)?,
                    message_count: row.get(2)?,
                })
            })
            .map_err(|e| CoreError::Store(format!("failed to query frontiers: {e}")))?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(
                row.map_err(|e| CoreError::Store(format!("failed to read frontier row: {e}")))?,
            );
        }
        Ok(entries)
    }

    pub fn get_messages_by_author(
        &self,
        chat_id: &ChatId,
        author_peer_id: &PeerId,
    ) -> Result<Vec<Message>, CoreError> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT message_id, chat_id, author_peer_id, lamport_ts, created_at,
                        key_epoch, parent_ids, signature, payload_ciphertext, payload_nonce, received_at
                 FROM messages
                 WHERE chat_id = ?1 AND author_peer_id = ?2
                 ORDER BY lamport_ts ASC",
            )
            .map_err(|e| {
                CoreError::Store(format!("failed to prepare get_messages_by_author: {e}"))
            })?;

        let rows = statement
            .query_map(
                rusqlite::params![chat_id.as_slice(), author_peer_id.as_slice()],
                |row| {
                    let parent_ids_blob: Option<Vec<u8>> = row.get(6)?;
                    Ok(MessageRow {
                        message_id: row.get::<_, Vec<u8>>(0)?,
                        chat_id: row.get::<_, Vec<u8>>(1)?,
                        author_peer_id: row.get::<_, Vec<u8>>(2)?,
                        lamport_ts: row.get(3)?,
                        created_at: row.get(4)?,
                        key_epoch: row.get(5)?,
                        parent_ids_blob,
                        signature: row.get(7)?,
                        payload_ciphertext: row.get(8)?,
                        payload_nonce: row.get::<_, Vec<u8>>(9)?,
                        received_at: row.get(10)?,
                    })
                },
            )
            .map_err(|e| CoreError::Store(format!("failed to query messages by author: {e}")))?;

        let mut messages = Vec::new();
        for row in rows {
            let raw = row
                .map_err(|e| CoreError::Store(format!("failed to read message row: {e}")))?;
            messages.push(message_row_to_message(raw)?);
        }
        Ok(messages)
    }

    pub fn get_messages_by_author_after(
        &self,
        chat_id: &ChatId,
        author_peer_id: &PeerId,
        after_lamport: u64,
    ) -> Result<Vec<Message>, CoreError> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT message_id, chat_id, author_peer_id, lamport_ts, created_at,
                        key_epoch, parent_ids, signature, payload_ciphertext, payload_nonce, received_at
                 FROM messages
                 WHERE chat_id = ?1 AND author_peer_id = ?2 AND lamport_ts > ?3
                 ORDER BY lamport_ts ASC",
            )
            .map_err(|e| {
                CoreError::Store(format!(
                    "failed to prepare get_messages_by_author_after: {e}"
                ))
            })?;

        let rows = statement
            .query_map(
                rusqlite::params![
                    chat_id.as_slice(),
                    author_peer_id.as_slice(),
                    after_lamport
                ],
                |row| {
                    let parent_ids_blob: Option<Vec<u8>> = row.get(6)?;
                    Ok(MessageRow {
                        message_id: row.get::<_, Vec<u8>>(0)?,
                        chat_id: row.get::<_, Vec<u8>>(1)?,
                        author_peer_id: row.get::<_, Vec<u8>>(2)?,
                        lamport_ts: row.get(3)?,
                        created_at: row.get(4)?,
                        key_epoch: row.get(5)?,
                        parent_ids_blob,
                        signature: row.get(7)?,
                        payload_ciphertext: row.get(8)?,
                        payload_nonce: row.get::<_, Vec<u8>>(9)?,
                        received_at: row.get(10)?,
                    })
                },
            )
            .map_err(|e| {
                CoreError::Store(format!("failed to query messages by author after: {e}"))
            })?;

        let mut messages = Vec::new();
        for row in rows {
            let raw = row
                .map_err(|e| CoreError::Store(format!("failed to read message row: {e}")))?;
            messages.push(message_row_to_message(raw)?);
        }
        Ok(messages)
    }

    pub fn get_last_message_timestamp(
        &self,
        chat_id: &ChatId,
    ) -> Result<Option<u64>, CoreError> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT created_at FROM messages
                 WHERE chat_id = ?1
                 ORDER BY lamport_ts DESC, author_peer_id DESC
                 LIMIT 1",
            )
            .map_err(|e| {
                CoreError::Store(format!(
                    "failed to prepare get_last_message_timestamp: {e}"
                ))
            })?;

        let mut rows = statement
            .query_map([chat_id.as_slice()], |row| row.get::<_, u64>(0))
            .map_err(|e| {
                CoreError::Store(format!(
                    "failed to query last message timestamp: {e}"
                ))
            })?;

        match rows.next() {
            Some(row) => Ok(Some(
                row.map_err(|e| {
                    CoreError::Store(format!(
                        "failed to read last message timestamp: {e}"
                    ))
                })?,
            )),
            None => Ok(None),
        }
    }

    pub fn update_frontier(
        &self,
        chat_id: &ChatId,
        author_peer_id: &PeerId,
        lamport_ts: u64,
    ) -> Result<(), CoreError> {
        self.connection()
            .execute(
                "INSERT INTO frontiers (chat_id, author_peer_id, max_lamport_ts, message_count)
                 VALUES (?1, ?2, ?3, 1)
                 ON CONFLICT(chat_id, author_peer_id) DO UPDATE SET
                     max_lamport_ts = MAX(excluded.max_lamport_ts, frontiers.max_lamport_ts),
                     message_count = frontiers.message_count + 1",
                rusqlite::params![
                    chat_id.as_slice(),
                    author_peer_id.as_slice(),
                    lamport_ts,
                ],
            )
            .map_err(|e| CoreError::Store(format!("failed to update frontier: {e}")))?;
        Ok(())
    }
}

// --- Internal helpers ---

struct MessageRow {
    message_id: Vec<u8>,
    chat_id: Vec<u8>,
    author_peer_id: Vec<u8>,
    lamport_ts: u64,
    created_at: u64,
    key_epoch: u64,
    parent_ids_blob: Option<Vec<u8>>,
    signature: Vec<u8>,
    payload_ciphertext: Vec<u8>,
    payload_nonce: Vec<u8>,
    received_at: u64,
}

fn message_row_to_message(row: MessageRow) -> Result<Message, CoreError> {
    let parent_ids = match row.parent_ids_blob {
        Some(blob) => decode_parent_ids(&blob)?,
        None => Vec::new(),
    };

    Ok(Message {
        message_id: blob_to_message_id(row.message_id),
        chat_id: blob_to_chat_id(row.chat_id),
        author_peer_id: blob_to_peer_id(row.author_peer_id),
        lamport_ts: row.lamport_ts,
        created_at: row.created_at,
        key_epoch: row.key_epoch,
        parent_ids,
        signature: row.signature,
        payload_ciphertext: row.payload_ciphertext,
        payload_nonce: blob_to_nonce(row.payload_nonce),
        received_at: row.received_at,
    })
}

fn encode_parent_ids(parent_ids: &[MessageId]) -> Result<Option<Vec<u8>>, CoreError> {
    if parent_ids.is_empty() {
        return Ok(None);
    }

    let ids_as_vecs: Vec<&[u8]> = parent_ids.iter().map(|id| id.as_slice()).collect();
    let mut buffer = Vec::new();
    ciborium::into_writer(&ids_as_vecs, &mut buffer)
        .map_err(|e| CoreError::Store(format!("failed to encode parent_ids as CBOR: {e}")))?;
    Ok(Some(buffer))
}

fn decode_parent_ids(blob: &[u8]) -> Result<Vec<MessageId>, CoreError> {
    let raw: Vec<Vec<u8>> = ciborium::from_reader(blob)
        .map_err(|e| CoreError::Store(format!("failed to decode parent_ids from CBOR: {e}")))?;

    let mut parent_ids = Vec::with_capacity(raw.len());
    for id_bytes in raw {
        parent_ids.push(blob_to_message_id(id_bytes));
    }
    Ok(parent_ids)
}

fn blob_to_peer_id(blob: Vec<u8>) -> PeerId {
    let mut peer_id = [0u8; 16];
    let len = blob.len().min(16);
    peer_id[..len].copy_from_slice(&blob[..len]);
    peer_id
}

fn blob_to_chat_id(blob: Vec<u8>) -> ChatId {
    let mut chat_id = [0u8; 16];
    let len = blob.len().min(16);
    chat_id[..len].copy_from_slice(&blob[..len]);
    chat_id
}

fn blob_to_message_id(blob: Vec<u8>) -> MessageId {
    let mut message_id = [0u8; 32];
    let len = blob.len().min(32);
    message_id[..len].copy_from_slice(&blob[..len]);
    message_id
}

fn blob_to_nonce(blob: Vec<u8>) -> [u8; 24] {
    let mut nonce = [0u8; 24];
    let len = blob.len().min(24);
    nonce[..len].copy_from_slice(&blob[..len]);
    nonce
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::db::Store;
    use crate::types::ChatId;

    fn test_store() -> Store {
        Store::open_in_memory().unwrap()
    }

    fn sample_chat_id() -> ChatId {
        [1u8; 16]
    }

    fn sample_peer_id() -> PeerId {
        [2u8; 16]
    }

    fn sample_message(lamport_ts: u64) -> Message {
        let mut message_id = [0u8; 32];
        message_id[0] = lamport_ts as u8;

        Message {
            message_id,
            chat_id: sample_chat_id(),
            author_peer_id: sample_peer_id(),
            lamport_ts,
            created_at: 1000 + lamport_ts,
            key_epoch: 0,
            parent_ids: Vec::new(),
            signature: vec![0xAA; 64],
            payload_ciphertext: vec![0xBB; 100],
            payload_nonce: [0xCC; 24],
            received_at: 2000 + lamport_ts,
        }
    }

    fn setup_chat(store: &Store) {
        use crate::types::Chat;
        store
            .insert_chat(&Chat {
                chat_id: sample_chat_id(),
                chat_name: "test".to_string(),
                owner_peer_id: sample_peer_id(),
                created_at: 1000,
                my_lamport_counter: 0,
                unread_count: 0,
            })
            .unwrap();
    }

    // --- Message CRUD ---

    #[test]
    fn insert_and_get_message() {
        let store = test_store();
        setup_chat(&store);

        let message = sample_message(1);
        store.insert_message(&message).unwrap();

        let loaded = store.get_message(&message.message_id).unwrap().unwrap();
        assert_eq!(loaded.message_id, message.message_id);
        assert_eq!(loaded.chat_id, message.chat_id);
        assert_eq!(loaded.author_peer_id, message.author_peer_id);
        assert_eq!(loaded.lamport_ts, 1);
        assert_eq!(loaded.key_epoch, 0);
        assert_eq!(loaded.signature, vec![0xAA; 64]);
        assert_eq!(loaded.payload_ciphertext, vec![0xBB; 100]);
        assert_eq!(loaded.payload_nonce, [0xCC; 24]);
    }

    #[test]
    fn get_message_returns_none_for_missing() {
        let store = test_store();
        let result = store.get_message(&[99u8; 32]).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn message_with_parent_ids_roundtrip() {
        let store = test_store();
        setup_chat(&store);

        let parent_a = [10u8; 32];
        let parent_b = [20u8; 32];

        let mut message = sample_message(1);
        message.parent_ids = vec![parent_a, parent_b];

        store.insert_message(&message).unwrap();
        let loaded = store.get_message(&message.message_id).unwrap().unwrap();

        assert_eq!(loaded.parent_ids.len(), 2);
        assert_eq!(loaded.parent_ids[0], parent_a);
        assert_eq!(loaded.parent_ids[1], parent_b);
    }

    #[test]
    fn message_with_empty_parent_ids() {
        let store = test_store();
        setup_chat(&store);

        let message = sample_message(1);
        store.insert_message(&message).unwrap();
        let loaded = store.get_message(&message.message_id).unwrap().unwrap();

        assert!(loaded.parent_ids.is_empty());
    }

    // --- Message query with ordering ---

    #[test]
    fn get_messages_ordered_by_lamport_ts() {
        let store = test_store();
        setup_chat(&store);

        // Insert out of order
        store.insert_message(&sample_message(3)).unwrap();
        store.insert_message(&sample_message(1)).unwrap();
        store.insert_message(&sample_message(2)).unwrap();

        let messages = store.get_messages(&sample_chat_id(), None, 100).unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].lamport_ts, 1);
        assert_eq!(messages[1].lamport_ts, 2);
        assert_eq!(messages[2].lamport_ts, 3);
    }

    #[test]
    fn get_messages_with_same_lamport_ordered_by_peer_id() {
        let store = test_store();
        setup_chat(&store);

        let peer_a = [1u8; 16];
        let peer_b = [2u8; 16];

        let mut msg_a = sample_message(1);
        msg_a.message_id = [0xAA; 32];
        msg_a.author_peer_id = peer_b;

        let mut msg_b = sample_message(1);
        msg_b.message_id = [0xBB; 32];
        msg_b.author_peer_id = peer_a;

        store.insert_message(&msg_b).unwrap();
        store.insert_message(&msg_a).unwrap();

        let messages = store.get_messages(&sample_chat_id(), None, 100).unwrap();
        assert_eq!(messages.len(), 2);
        // peer_a ([1;16]) comes before peer_b ([2;16]) in byte order
        assert_eq!(messages[0].author_peer_id, peer_a);
        assert_eq!(messages[1].author_peer_id, peer_b);
    }

    #[test]
    fn get_messages_with_before_lamport() {
        let store = test_store();
        setup_chat(&store);

        for ts in 1..=5 {
            let mut msg = sample_message(ts);
            msg.message_id[1] = ts as u8;
            store.insert_message(&msg).unwrap();
        }

        let messages = store.get_messages(&sample_chat_id(), Some(3), 100).unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].lamport_ts, 1);
        assert_eq!(messages[1].lamport_ts, 2);
    }

    #[test]
    fn get_messages_with_limit() {
        let store = test_store();
        setup_chat(&store);

        for ts in 1..=10 {
            let mut msg = sample_message(ts);
            msg.message_id[1] = ts as u8;
            store.insert_message(&msg).unwrap();
        }

        let messages = store.get_messages(&sample_chat_id(), None, 3).unwrap();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].lamport_ts, 1);
        assert_eq!(messages[2].lamport_ts, 3);
    }

    #[test]
    fn get_messages_empty_chat() {
        let store = test_store();
        let messages = store.get_messages(&sample_chat_id(), None, 100).unwrap();
        assert!(messages.is_empty());
    }

    // --- Frontier ---

    #[test]
    fn insert_message_creates_frontier() {
        let store = test_store();
        setup_chat(&store);

        store.insert_message(&sample_message(5)).unwrap();

        let frontier = store.get_frontier(&sample_chat_id()).unwrap();
        assert_eq!(frontier.len(), 1);
        assert_eq!(frontier[0].author_peer_id, sample_peer_id());
        assert_eq!(frontier[0].max_lamport_ts, 5);
        assert_eq!(frontier[0].message_count, 1);
    }

    #[test]
    fn insert_multiple_messages_updates_frontier() {
        let store = test_store();
        setup_chat(&store);

        let mut msg1 = sample_message(3);
        msg1.message_id = [0x01; 32];
        let mut msg2 = sample_message(7);
        msg2.message_id = [0x02; 32];

        store.insert_message(&msg1).unwrap();
        store.insert_message(&msg2).unwrap();

        let frontier = store.get_frontier(&sample_chat_id()).unwrap();
        assert_eq!(frontier.len(), 1);
        assert_eq!(frontier[0].max_lamport_ts, 7);
        assert_eq!(frontier[0].message_count, 2);
    }

    #[test]
    fn frontier_tracks_max_lamport_correctly() {
        let store = test_store();
        setup_chat(&store);

        // Insert higher first, then lower — max should stay at higher
        let mut msg1 = sample_message(10);
        msg1.message_id = [0x01; 32];
        let mut msg2 = sample_message(5);
        msg2.message_id = [0x02; 32];

        store.insert_message(&msg1).unwrap();
        store.insert_message(&msg2).unwrap();

        let frontier = store.get_frontier(&sample_chat_id()).unwrap();
        assert_eq!(frontier[0].max_lamport_ts, 10);
        assert_eq!(frontier[0].message_count, 2);
    }

    #[test]
    fn frontier_per_author() {
        let store = test_store();
        setup_chat(&store);

        let peer_a = [0xAA; 16];
        let peer_b = [0xBB; 16];

        let mut msg_a = sample_message(1);
        msg_a.message_id = [0x01; 32];
        msg_a.author_peer_id = peer_a;

        let mut msg_b = sample_message(2);
        msg_b.message_id = [0x02; 32];
        msg_b.author_peer_id = peer_b;

        store.insert_message(&msg_a).unwrap();
        store.insert_message(&msg_b).unwrap();

        let frontier = store.get_frontier(&sample_chat_id()).unwrap();
        assert_eq!(frontier.len(), 2);
    }

    // --- Messages by author ---

    #[test]
    fn get_messages_by_author_returns_only_matching() {
        let store = test_store();
        setup_chat(&store);

        let peer_a = [0xAA; 16];
        let peer_b = [0xBB; 16];

        let mut msg_a = sample_message(1);
        msg_a.message_id = [0x01; 32];
        msg_a.author_peer_id = peer_a;

        let mut msg_b = sample_message(2);
        msg_b.message_id = [0x02; 32];
        msg_b.author_peer_id = peer_b;

        let mut msg_a2 = sample_message(3);
        msg_a2.message_id = [0x03; 32];
        msg_a2.author_peer_id = peer_a;

        store.insert_message(&msg_a).unwrap();
        store.insert_message(&msg_b).unwrap();
        store.insert_message(&msg_a2).unwrap();

        let messages = store
            .get_messages_by_author(&sample_chat_id(), &peer_a)
            .unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].lamport_ts, 1);
        assert_eq!(messages[1].lamport_ts, 3);
    }

    #[test]
    fn get_messages_by_author_empty_result() {
        let store = test_store();
        setup_chat(&store);

        store.insert_message(&sample_message(1)).unwrap();

        let messages = store
            .get_messages_by_author(&sample_chat_id(), &[0xFF; 16])
            .unwrap();
        assert!(messages.is_empty());
    }

    #[test]
    fn get_messages_by_author_after_filters_by_lamport() {
        let store = test_store();
        setup_chat(&store);

        let peer = sample_peer_id();
        for ts in 1..=5 {
            let mut msg = sample_message(ts);
            msg.message_id[1] = ts as u8;
            store.insert_message(&msg).unwrap();
        }

        let messages = store
            .get_messages_by_author_after(&sample_chat_id(), &peer, 3)
            .unwrap();
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[0].lamport_ts, 4);
        assert_eq!(messages[1].lamport_ts, 5);
    }

    #[test]
    fn get_messages_by_author_after_with_zero_returns_all() {
        let store = test_store();
        setup_chat(&store);

        for ts in 1..=3 {
            let mut msg = sample_message(ts);
            msg.message_id[1] = ts as u8;
            store.insert_message(&msg).unwrap();
        }

        let messages = store
            .get_messages_by_author_after(&sample_chat_id(), &sample_peer_id(), 0)
            .unwrap();
        assert_eq!(messages.len(), 3);
    }

    #[test]
    fn get_messages_by_author_after_beyond_max_returns_empty() {
        let store = test_store();
        setup_chat(&store);

        store.insert_message(&sample_message(5)).unwrap();

        let messages = store
            .get_messages_by_author_after(&sample_chat_id(), &sample_peer_id(), 10)
            .unwrap();
        assert!(messages.is_empty());
    }

    #[test]
    fn get_frontier_empty_chat() {
        let store = test_store();
        let frontier = store.get_frontier(&sample_chat_id()).unwrap();
        assert!(frontier.is_empty());
    }

    // --- Last message timestamp ---

    #[test]
    fn get_last_message_timestamp_returns_latest() {
        let store = test_store();
        setup_chat(&store);

        let mut msg1 = sample_message(1);
        msg1.message_id = [0x01; 32];
        msg1.created_at = 5000;
        let mut msg2 = sample_message(3);
        msg2.message_id = [0x02; 32];
        msg2.created_at = 7000;
        let mut msg3 = sample_message(2);
        msg3.message_id = [0x03; 32];
        msg3.created_at = 6000;

        store.insert_message(&msg1).unwrap();
        store.insert_message(&msg2).unwrap();
        store.insert_message(&msg3).unwrap();

        let timestamp = store
            .get_last_message_timestamp(&sample_chat_id())
            .unwrap()
            .unwrap();
        // msg2 has the highest lamport_ts (3), so its created_at (7000) is returned
        assert_eq!(timestamp, 7000);
    }

    #[test]
    fn get_last_message_timestamp_empty_chat() {
        let store = test_store();
        let result = store
            .get_last_message_timestamp(&sample_chat_id())
            .unwrap();
        assert!(result.is_none());
    }

    // --- CBOR parent_ids encoding ---

    #[test]
    fn encode_decode_parent_ids_roundtrip() {
        let ids = vec![[1u8; 32], [2u8; 32], [3u8; 32]];
        let encoded = encode_parent_ids(&ids).unwrap().unwrap();
        let decoded = decode_parent_ids(&encoded).unwrap();
        assert_eq!(decoded, ids);
    }

    #[test]
    fn encode_empty_parent_ids_returns_none() {
        let result = encode_parent_ids(&[]).unwrap();
        assert!(result.is_none());
    }
}

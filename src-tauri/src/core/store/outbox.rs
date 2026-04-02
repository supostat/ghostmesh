use crate::types::{
    ChatId, CoreError, MessageId, OutboxEntry, PeerAddress, PeerId, SyncLogEntry,
};

use super::db::Store;

const SYNC_LOG_MAX_RECORDS: i64 = 1000;

impl Store {
    // --- Outbox ---

    pub fn insert_outbox_entry(&self, entry: &OutboxEntry) -> Result<(), CoreError> {
        self.connection()
            .execute(
                "INSERT INTO outbox (message_id, target_peer_id, chat_id, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    entry.message_id.as_slice(),
                    entry.target_peer_id.as_slice(),
                    entry.chat_id.as_slice(),
                    entry.created_at,
                ],
            )
            .map_err(|e| CoreError::Store(format!("failed to insert outbox entry: {e}")))?;
        Ok(())
    }

    pub fn get_outbox_for_peer(
        &self,
        peer_id: &PeerId,
    ) -> Result<Vec<OutboxEntry>, CoreError> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT message_id, target_peer_id, chat_id, created_at
                 FROM outbox WHERE target_peer_id = ?1
                 ORDER BY created_at ASC",
            )
            .map_err(|e| CoreError::Store(format!("failed to prepare get_outbox_for_peer: {e}")))?;

        let rows = statement
            .query_map([peer_id.as_slice()], |row| {
                Ok(OutboxEntry {
                    message_id: blob_to_message_id(row.get::<_, Vec<u8>>(0)?),
                    target_peer_id: blob_to_peer_id(row.get::<_, Vec<u8>>(1)?),
                    chat_id: blob_to_chat_id(row.get::<_, Vec<u8>>(2)?),
                    created_at: row.get(3)?,
                })
            })
            .map_err(|e| CoreError::Store(format!("failed to query outbox for peer: {e}")))?;

        collect_rows(rows, "outbox")
    }

    pub fn get_outbox_for_chat(
        &self,
        chat_id: &ChatId,
    ) -> Result<Vec<OutboxEntry>, CoreError> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT message_id, target_peer_id, chat_id, created_at
                 FROM outbox WHERE chat_id = ?1
                 ORDER BY created_at ASC",
            )
            .map_err(|e| CoreError::Store(format!("failed to prepare get_outbox_for_chat: {e}")))?;

        let rows = statement
            .query_map([chat_id.as_slice()], |row| {
                Ok(OutboxEntry {
                    message_id: blob_to_message_id(row.get::<_, Vec<u8>>(0)?),
                    target_peer_id: blob_to_peer_id(row.get::<_, Vec<u8>>(1)?),
                    chat_id: blob_to_chat_id(row.get::<_, Vec<u8>>(2)?),
                    created_at: row.get(3)?,
                })
            })
            .map_err(|e| CoreError::Store(format!("failed to query outbox for chat: {e}")))?;

        collect_rows(rows, "outbox")
    }

    pub fn delete_outbox_entry(
        &self,
        message_id: &MessageId,
        target_peer_id: &PeerId,
    ) -> Result<(), CoreError> {
        self.connection()
            .execute(
                "DELETE FROM outbox WHERE message_id = ?1 AND target_peer_id = ?2",
                rusqlite::params![message_id.as_slice(), target_peer_id.as_slice()],
            )
            .map_err(|e| CoreError::Store(format!("failed to delete outbox entry: {e}")))?;
        Ok(())
    }

    // --- Peer Addresses ---

    pub fn upsert_peer_address(&self, address: &PeerAddress) -> Result<(), CoreError> {
        self.connection()
            .execute(
                "INSERT INTO peer_addresses
                 (peer_id, address_type, address, last_seen, last_successful, fail_count)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT(peer_id, address_type, address) DO UPDATE SET
                     last_seen = excluded.last_seen,
                     last_successful = COALESCE(excluded.last_successful, peer_addresses.last_successful),
                     fail_count = excluded.fail_count",
                rusqlite::params![
                    address.peer_id.as_slice(),
                    address.address_type,
                    address.address,
                    address.last_seen,
                    address.last_successful,
                    address.fail_count,
                ],
            )
            .map_err(|e| CoreError::Store(format!("failed to upsert peer address: {e}")))?;
        Ok(())
    }

    pub fn get_peer_addresses(
        &self,
        peer_id: &PeerId,
    ) -> Result<Vec<PeerAddress>, CoreError> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT peer_id, address_type, address, last_seen, last_successful, fail_count
                 FROM peer_addresses WHERE peer_id = ?1",
            )
            .map_err(|e| {
                CoreError::Store(format!("failed to prepare get_peer_addresses: {e}"))
            })?;

        let rows = statement
            .query_map([peer_id.as_slice()], row_to_peer_address)
            .map_err(|e| CoreError::Store(format!("failed to query peer addresses: {e}")))?;

        collect_rows(rows, "peer_addresses")
    }

    pub fn get_all_peer_addresses(&self) -> Result<Vec<PeerAddress>, CoreError> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT peer_id, address_type, address, last_seen, last_successful, fail_count
                 FROM peer_addresses",
            )
            .map_err(|e| {
                CoreError::Store(format!("failed to prepare get_all_peer_addresses: {e}"))
            })?;

        let rows = statement
            .query_map([], row_to_peer_address)
            .map_err(|e| CoreError::Store(format!("failed to query all peer addresses: {e}")))?;

        collect_rows(rows, "peer_addresses")
    }

    pub fn cleanup_stale_peer_addresses(
        &self,
        max_age_secs: u64,
        max_fail_count: u32,
    ) -> Result<u64, CoreError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_secs();
        let cutoff = now.saturating_sub(max_age_secs);

        let deleted = self
            .connection()
            .execute(
                "DELETE FROM peer_addresses
                 WHERE fail_count > ?1 AND last_seen < ?2",
                rusqlite::params![max_fail_count, cutoff],
            )
            .map_err(|e| {
                CoreError::Store(format!("failed to cleanup stale peer addresses: {e}"))
            })?;

        Ok(deleted as u64)
    }

    // --- Sync Log ---

    pub fn insert_sync_log(&self, entry: &SyncLogEntry) -> Result<(), CoreError> {
        self.connection()
            .execute(
                "INSERT INTO sync_log (timestamp, peer_id, event_type, detail)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    entry.timestamp,
                    entry.peer_id.map(|p| p.to_vec()),
                    entry.event_type,
                    entry.detail,
                ],
            )
            .map_err(|e| CoreError::Store(format!("failed to insert sync log: {e}")))?;

        self.enforce_sync_log_limit()?;

        Ok(())
    }

    pub fn get_sync_log(&self, limit: u32) -> Result<Vec<SyncLogEntry>, CoreError> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT id, timestamp, peer_id, event_type, detail
                 FROM sync_log ORDER BY timestamp DESC, id DESC LIMIT ?1",
            )
            .map_err(|e| CoreError::Store(format!("failed to prepare get_sync_log: {e}")))?;

        let rows = statement
            .query_map([limit], |row| {
                let peer_id_blob: Option<Vec<u8>> = row.get(2)?;
                Ok(SyncLogEntry {
                    id: row.get(0)?,
                    timestamp: row.get(1)?,
                    peer_id: peer_id_blob.map(blob_to_peer_id),
                    event_type: row.get(3)?,
                    detail: row.get(4)?,
                })
            })
            .map_err(|e| CoreError::Store(format!("failed to query sync log: {e}")))?;

        collect_rows(rows, "sync_log")
    }

    fn enforce_sync_log_limit(&self) -> Result<(), CoreError> {
        self.connection()
            .execute(
                "DELETE FROM sync_log WHERE id NOT IN
                 (SELECT id FROM sync_log ORDER BY id DESC LIMIT ?1)",
                [SYNC_LOG_MAX_RECORDS],
            )
            .map_err(|e| CoreError::Store(format!("failed to enforce sync log limit: {e}")))?;
        Ok(())
    }
}

// --- Internal helpers ---

fn row_to_peer_address(row: &rusqlite::Row<'_>) -> rusqlite::Result<PeerAddress> {
    Ok(PeerAddress {
        peer_id: blob_to_peer_id(row.get::<_, Vec<u8>>(0)?),
        address_type: row.get(1)?,
        address: row.get(2)?,
        last_seen: row.get(3)?,
        last_successful: row.get(4)?,
        fail_count: row.get(5)?,
    })
}

fn collect_rows<T>(
    rows: rusqlite::MappedRows<'_, impl FnMut(&rusqlite::Row<'_>) -> rusqlite::Result<T>>,
    table_name: &str,
) -> Result<Vec<T>, CoreError> {
    let mut results = Vec::new();
    for row in rows {
        results.push(
            row.map_err(|e| {
                CoreError::Store(format!("failed to read {table_name} row: {e}"))
            })?,
        );
    }
    Ok(results)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::db::Store;

    fn test_store() -> Store {
        Store::open_in_memory().unwrap()
    }

    fn sample_peer_id() -> PeerId {
        [2u8; 16]
    }

    fn sample_chat_id() -> ChatId {
        [1u8; 16]
    }

    fn sample_message_id(n: u8) -> MessageId {
        let mut id = [0u8; 32];
        id[0] = n;
        id
    }

    fn sample_outbox_entry(message_n: u8, target: PeerId) -> OutboxEntry {
        OutboxEntry {
            message_id: sample_message_id(message_n),
            target_peer_id: target,
            chat_id: sample_chat_id(),
            created_at: 1000 + message_n as u64,
        }
    }

    // --- Outbox ---

    #[test]
    fn insert_and_get_outbox_for_peer() {
        let store = test_store();
        let target = [3u8; 16];

        store
            .insert_outbox_entry(&sample_outbox_entry(1, target))
            .unwrap();
        store
            .insert_outbox_entry(&sample_outbox_entry(2, target))
            .unwrap();

        let entries = store.get_outbox_for_peer(&target).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].message_id, sample_message_id(1));
        assert_eq!(entries[1].message_id, sample_message_id(2));
    }

    #[test]
    fn get_outbox_for_chat() {
        let store = test_store();
        let target_a = [3u8; 16];
        let target_b = [4u8; 16];

        store
            .insert_outbox_entry(&sample_outbox_entry(1, target_a))
            .unwrap();
        store
            .insert_outbox_entry(&sample_outbox_entry(2, target_b))
            .unwrap();

        let entries = store.get_outbox_for_chat(&sample_chat_id()).unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn delete_outbox_entry_removes_specific() {
        let store = test_store();
        let target = [3u8; 16];

        store
            .insert_outbox_entry(&sample_outbox_entry(1, target))
            .unwrap();
        store
            .insert_outbox_entry(&sample_outbox_entry(2, target))
            .unwrap();

        store
            .delete_outbox_entry(&sample_message_id(1), &target)
            .unwrap();

        let entries = store.get_outbox_for_peer(&target).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].message_id, sample_message_id(2));
    }

    #[test]
    fn get_outbox_empty() {
        let store = test_store();
        let entries = store.get_outbox_for_peer(&[99u8; 16]).unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn outbox_roundtrip_preserves_fields() {
        let store = test_store();
        let entry = OutboxEntry {
            message_id: [0xAA; 32],
            target_peer_id: [0xBB; 16],
            chat_id: [0xCC; 16],
            created_at: 42,
        };
        store.insert_outbox_entry(&entry).unwrap();

        let loaded = store.get_outbox_for_peer(&[0xBB; 16]).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].message_id, [0xAA; 32]);
        assert_eq!(loaded[0].target_peer_id, [0xBB; 16]);
        assert_eq!(loaded[0].chat_id, [0xCC; 16]);
        assert_eq!(loaded[0].created_at, 42);
    }

    // --- Peer Addresses ---

    #[test]
    fn upsert_and_get_peer_addresses() {
        let store = test_store();
        let address = PeerAddress {
            peer_id: sample_peer_id(),
            address_type: "tcp".to_string(),
            address: "192.168.1.1:9473".to_string(),
            last_seen: 1000,
            last_successful: Some(900),
            fail_count: 0,
        };

        store.upsert_peer_address(&address).unwrap();
        let loaded = store.get_peer_addresses(&sample_peer_id()).unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].address_type, "tcp");
        assert_eq!(loaded[0].address, "192.168.1.1:9473");
        assert_eq!(loaded[0].last_seen, 1000);
        assert_eq!(loaded[0].last_successful, Some(900));
        assert_eq!(loaded[0].fail_count, 0);
    }

    #[test]
    fn upsert_peer_address_updates_existing() {
        let store = test_store();
        let address_v1 = PeerAddress {
            peer_id: sample_peer_id(),
            address_type: "tcp".to_string(),
            address: "192.168.1.1:9473".to_string(),
            last_seen: 1000,
            last_successful: Some(900),
            fail_count: 0,
        };
        store.upsert_peer_address(&address_v1).unwrap();

        let address_v2 = PeerAddress {
            peer_id: sample_peer_id(),
            address_type: "tcp".to_string(),
            address: "192.168.1.1:9473".to_string(),
            last_seen: 2000,
            last_successful: None,
            fail_count: 3,
        };
        store.upsert_peer_address(&address_v2).unwrap();

        let loaded = store.get_peer_addresses(&sample_peer_id()).unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].last_seen, 2000);
        // COALESCE keeps previous last_successful when new is NULL
        assert_eq!(loaded[0].last_successful, Some(900));
        assert_eq!(loaded[0].fail_count, 3);
    }

    #[test]
    fn get_all_peer_addresses() {
        let store = test_store();

        let addr1 = PeerAddress {
            peer_id: [1u8; 16],
            address_type: "tcp".to_string(),
            address: "1.1.1.1:9473".to_string(),
            last_seen: 1000,
            last_successful: None,
            fail_count: 0,
        };
        let addr2 = PeerAddress {
            peer_id: [2u8; 16],
            address_type: "tcp".to_string(),
            address: "2.2.2.2:9473".to_string(),
            last_seen: 2000,
            last_successful: None,
            fail_count: 0,
        };

        store.upsert_peer_address(&addr1).unwrap();
        store.upsert_peer_address(&addr2).unwrap();

        let all = store.get_all_peer_addresses().unwrap();
        assert_eq!(all.len(), 2);
    }

    #[test]
    fn peer_address_with_none_last_successful() {
        let store = test_store();
        let address = PeerAddress {
            peer_id: sample_peer_id(),
            address_type: "mdns".to_string(),
            address: "local.mesh:9473".to_string(),
            last_seen: 500,
            last_successful: None,
            fail_count: 1,
        };

        store.upsert_peer_address(&address).unwrap();
        let loaded = store.get_peer_addresses(&sample_peer_id()).unwrap();
        assert_eq!(loaded[0].last_successful, None);
    }

    #[test]
    fn cleanup_stale_peer_addresses_removes_old_failures() {
        let store = test_store();

        // Stale address: high fail count, old last_seen
        let stale = PeerAddress {
            peer_id: [1u8; 16],
            address_type: "tcp".to_string(),
            address: "old.host:9473".to_string(),
            last_seen: 100, // very old
            last_successful: None,
            fail_count: 15,
        };
        store.upsert_peer_address(&stale).unwrap();

        // Fresh address: same fail count but recently seen
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let fresh = PeerAddress {
            peer_id: [2u8; 16],
            address_type: "tcp".to_string(),
            address: "new.host:9473".to_string(),
            last_seen: now,
            last_successful: None,
            fail_count: 15,
        };
        store.upsert_peer_address(&fresh).unwrap();

        // max_age_secs=30*24*3600 (30 days), max_fail_count=10
        let deleted = store
            .cleanup_stale_peer_addresses(30 * 24 * 3600, 10)
            .unwrap();
        assert_eq!(deleted, 1);

        let remaining = store.get_all_peer_addresses().unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].peer_id, [2u8; 16]);
    }

    // --- Sync Log ---

    #[test]
    fn insert_and_get_sync_log() {
        let store = test_store();

        let entry = SyncLogEntry {
            id: 0, // auto-assigned
            timestamp: 1000,
            peer_id: Some(sample_peer_id()),
            event_type: "connect".to_string(),
            detail: Some("handshake complete".to_string()),
        };

        store.insert_sync_log(&entry).unwrap();
        let log = store.get_sync_log(10).unwrap();

        assert_eq!(log.len(), 1);
        assert_eq!(log[0].timestamp, 1000);
        assert_eq!(log[0].peer_id, Some(sample_peer_id()));
        assert_eq!(log[0].event_type, "connect");
        assert_eq!(log[0].detail.as_deref(), Some("handshake complete"));
    }

    #[test]
    fn sync_log_without_peer_id() {
        let store = test_store();

        let entry = SyncLogEntry {
            id: 0,
            timestamp: 2000,
            peer_id: None,
            event_type: "sync".to_string(),
            detail: None,
        };

        store.insert_sync_log(&entry).unwrap();
        let log = store.get_sync_log(10).unwrap();

        assert_eq!(log[0].peer_id, None);
        assert_eq!(log[0].detail, None);
    }

    #[test]
    fn sync_log_newest_first() {
        let store = test_store();

        for ts in [100, 300, 200] {
            store
                .insert_sync_log(&SyncLogEntry {
                    id: 0,
                    timestamp: ts,
                    peer_id: None,
                    event_type: "sync".to_string(),
                    detail: None,
                })
                .unwrap();
        }

        let log = store.get_sync_log(10).unwrap();
        assert_eq!(log.len(), 3);
        assert_eq!(log[0].timestamp, 300);
        assert_eq!(log[1].timestamp, 200);
        assert_eq!(log[2].timestamp, 100);
    }

    #[test]
    fn sync_log_ring_buffer_enforces_1000_limit() {
        let store = test_store();

        for i in 0..1005 {
            store
                .insert_sync_log(&SyncLogEntry {
                    id: 0,
                    timestamp: i as u64,
                    peer_id: None,
                    event_type: "sync".to_string(),
                    detail: Some(format!("entry-{i}")),
                })
                .unwrap();
        }

        let log = store.get_sync_log(2000).unwrap();
        assert_eq!(log.len(), 1000);

        // Newest entries are kept
        assert_eq!(log[0].timestamp, 1004);
        // Oldest surviving entry
        assert_eq!(log[999].timestamp, 5);
    }

    #[test]
    fn sync_log_limit_param() {
        let store = test_store();

        for i in 0..10 {
            store
                .insert_sync_log(&SyncLogEntry {
                    id: 0,
                    timestamp: i,
                    peer_id: None,
                    event_type: "sync".to_string(),
                    detail: None,
                })
                .unwrap();
        }

        let log = store.get_sync_log(3).unwrap();
        assert_eq!(log.len(), 3);
    }
}

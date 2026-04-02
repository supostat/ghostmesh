pub mod db;
pub mod chats;
pub mod messages;
pub mod outbox;

pub use db::Store;

use crate::types::{CoreError, PeerId};

#[derive(Debug, Clone)]
pub struct StoredIdentity {
    pub peer_id: PeerId,
    pub signing_sk_enc: Vec<u8>,
    pub signing_pk: [u8; 32],
    pub exchange_sk_enc: Vec<u8>,
    pub exchange_pk: [u8; 32],
    pub display_name: String,
    pub created_at: u64,
}

impl Store {
    pub fn save_identity(
        &self,
        peer_id: &PeerId,
        signing_sk_enc: &[u8],
        signing_pk: &[u8; 32],
        exchange_sk_enc: &[u8],
        exchange_pk: &[u8; 32],
        display_name: &str,
        created_at: u64,
    ) -> Result<(), CoreError> {
        self.connection()
            .execute(
                "INSERT OR REPLACE INTO identity
                 (peer_id, signing_sk_enc, signing_pk, exchange_sk_enc, exchange_pk, display_name, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                rusqlite::params![
                    peer_id.as_slice(),
                    signing_sk_enc,
                    signing_pk.as_slice(),
                    exchange_sk_enc,
                    exchange_pk.as_slice(),
                    display_name,
                    created_at,
                ],
            )
            .map_err(|e| CoreError::Store(format!("failed to save identity: {e}")))?;
        Ok(())
    }

    pub fn get_identity(&self) -> Result<Option<StoredIdentity>, CoreError> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT peer_id, signing_sk_enc, signing_pk, exchange_sk_enc, exchange_pk,
                        display_name, created_at
                 FROM identity LIMIT 1",
            )
            .map_err(|e| CoreError::Store(format!("failed to prepare get_identity: {e}")))?;

        let mut rows = statement
            .query_map([], |row| {
                Ok(StoredIdentityRow {
                    peer_id: row.get::<_, Vec<u8>>(0)?,
                    signing_sk_enc: row.get(1)?,
                    signing_pk: row.get::<_, Vec<u8>>(2)?,
                    exchange_sk_enc: row.get(3)?,
                    exchange_pk: row.get::<_, Vec<u8>>(4)?,
                    display_name: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .map_err(|e| CoreError::Store(format!("failed to query identity: {e}")))?;

        match rows.next() {
            Some(row) => {
                let raw = row
                    .map_err(|e| CoreError::Store(format!("failed to read identity row: {e}")))?;
                Ok(Some(StoredIdentity {
                    peer_id: blob_to_peer_id(raw.peer_id),
                    signing_sk_enc: raw.signing_sk_enc,
                    signing_pk: blob_to_key32(raw.signing_pk),
                    exchange_sk_enc: raw.exchange_sk_enc,
                    exchange_pk: blob_to_key32(raw.exchange_pk),
                    display_name: raw.display_name,
                    created_at: raw.created_at,
                }))
            }
            None => Ok(None),
        }
    }
}

struct StoredIdentityRow {
    peer_id: Vec<u8>,
    signing_sk_enc: Vec<u8>,
    signing_pk: Vec<u8>,
    exchange_sk_enc: Vec<u8>,
    exchange_pk: Vec<u8>,
    display_name: String,
    created_at: u64,
}

fn blob_to_peer_id(blob: Vec<u8>) -> PeerId {
    let mut peer_id = [0u8; 16];
    let len = blob.len().min(16);
    peer_id[..len].copy_from_slice(&blob[..len]);
    peer_id
}

fn blob_to_key32(blob: Vec<u8>) -> [u8; 32] {
    let mut key = [0u8; 32];
    let len = blob.len().min(32);
    key[..len].copy_from_slice(&blob[..len]);
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_store() -> Store {
        Store::open_in_memory().unwrap()
    }

    #[test]
    fn save_and_get_identity() {
        let store = test_store();
        let peer_id: PeerId = [1u8; 16];
        let signing_sk_enc = vec![0xAA; 80];
        let signing_pk = [2u8; 32];
        let exchange_sk_enc = vec![0xBB; 64];
        let exchange_pk = [3u8; 32];

        store
            .save_identity(
                &peer_id,
                &signing_sk_enc,
                &signing_pk,
                &exchange_sk_enc,
                &exchange_pk,
                "alice",
                1000,
            )
            .unwrap();

        let loaded = store.get_identity().unwrap().unwrap();

        assert_eq!(loaded.peer_id, peer_id);
        assert_eq!(loaded.signing_sk_enc, signing_sk_enc);
        assert_eq!(loaded.signing_pk, signing_pk);
        assert_eq!(loaded.exchange_sk_enc, exchange_sk_enc);
        assert_eq!(loaded.exchange_pk, exchange_pk);
        assert_eq!(loaded.display_name, "alice");
        assert_eq!(loaded.created_at, 1000);
    }

    #[test]
    fn get_identity_returns_none_when_empty() {
        let store = test_store();
        let result = store.get_identity().unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn save_identity_replaces_existing() {
        let store = test_store();

        store
            .save_identity(
                &[1u8; 16],
                &[0xAA; 80],
                &[2u8; 32],
                &[0xBB; 64],
                &[3u8; 32],
                "alice",
                1000,
            )
            .unwrap();

        store
            .save_identity(
                &[1u8; 16],
                &[0xCC; 80],
                &[4u8; 32],
                &[0xDD; 64],
                &[5u8; 32],
                "alice-updated",
                2000,
            )
            .unwrap();

        let loaded = store.get_identity().unwrap().unwrap();
        assert_eq!(loaded.display_name, "alice-updated");
        assert_eq!(loaded.signing_pk, [4u8; 32]);
        assert_eq!(loaded.created_at, 2000);
    }
}

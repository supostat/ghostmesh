use crate::types::{Chat, ChatId, ChatKey, ChatMember, CoreError, MemberRole, PeerId};

use super::db::Store;

impl Store {
    pub fn insert_chat(&self, chat: &Chat) -> Result<(), CoreError> {
        self.connection()
            .execute(
                "INSERT INTO chats (chat_id, chat_name, owner_peer_id, created_at, my_lamport_counter)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                rusqlite::params![
                    chat.chat_id.as_slice(),
                    chat.chat_name,
                    chat.owner_peer_id.as_slice(),
                    chat.created_at,
                    chat.my_lamport_counter,
                ],
            )
            .map_err(|e| CoreError::Store(format!("failed to insert chat: {e}")))?;
        Ok(())
    }

    pub fn get_chat(&self, chat_id: &ChatId) -> Result<Option<Chat>, CoreError> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT chat_id, chat_name, owner_peer_id, created_at, my_lamport_counter
                 FROM chats WHERE chat_id = ?1",
            )
            .map_err(|e| CoreError::Store(format!("failed to prepare get_chat: {e}")))?;

        let mut rows = statement
            .query_map([chat_id.as_slice()], |row| {
                Ok(Chat {
                    chat_id: blob_to_chat_id(row.get::<_, Vec<u8>>(0)?),
                    chat_name: row.get(1)?,
                    owner_peer_id: blob_to_peer_id(row.get::<_, Vec<u8>>(2)?),
                    created_at: row.get(3)?,
                    my_lamport_counter: row.get(4)?,
                })
            })
            .map_err(|e| CoreError::Store(format!("failed to query chat: {e}")))?;

        match rows.next() {
            Some(row) => Ok(Some(
                row.map_err(|e| CoreError::Store(format!("failed to read chat row: {e}")))?,
            )),
            None => Ok(None),
        }
    }

    pub fn list_chats(&self) -> Result<Vec<Chat>, CoreError> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT chat_id, chat_name, owner_peer_id, created_at, my_lamport_counter
                 FROM chats ORDER BY created_at DESC",
            )
            .map_err(|e| CoreError::Store(format!("failed to prepare list_chats: {e}")))?;

        let rows = statement
            .query_map([], |row| {
                Ok(Chat {
                    chat_id: blob_to_chat_id(row.get::<_, Vec<u8>>(0)?),
                    chat_name: row.get(1)?,
                    owner_peer_id: blob_to_peer_id(row.get::<_, Vec<u8>>(2)?),
                    created_at: row.get(3)?,
                    my_lamport_counter: row.get(4)?,
                })
            })
            .map_err(|e| CoreError::Store(format!("failed to query chats: {e}")))?;

        let mut chats = Vec::new();
        for row in rows {
            chats.push(
                row.map_err(|e| CoreError::Store(format!("failed to read chat row: {e}")))?,
            );
        }
        Ok(chats)
    }

    pub fn insert_chat_member(&self, member: &ChatMember) -> Result<(), CoreError> {
        self.connection()
            .execute(
                "INSERT INTO chat_members
                 (chat_id, peer_id, signing_pk, exchange_pk, display_name, role, added_at, added_by, is_removed)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                rusqlite::params![
                    member.chat_id.as_slice(),
                    member.peer_id.as_slice(),
                    member.signing_pk.as_slice(),
                    member.exchange_pk.as_slice(),
                    member.display_name,
                    member.role.as_str(),
                    member.added_at,
                    member.added_by.as_slice(),
                    member.is_removed as i32,
                ],
            )
            .map_err(|e| CoreError::Store(format!("failed to insert chat member: {e}")))?;
        Ok(())
    }

    pub fn get_chat_members(&self, chat_id: &ChatId) -> Result<Vec<ChatMember>, CoreError> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT chat_id, peer_id, signing_pk, exchange_pk, display_name,
                        role, added_at, added_by, is_removed
                 FROM chat_members WHERE chat_id = ?1 ORDER BY added_at ASC",
            )
            .map_err(|e| CoreError::Store(format!("failed to prepare get_chat_members: {e}")))?;

        let rows = statement
            .query_map([chat_id.as_slice()], |row| {
                let role_str: String = row.get(5)?;
                Ok(ChatMember {
                    chat_id: blob_to_chat_id(row.get::<_, Vec<u8>>(0)?),
                    peer_id: blob_to_peer_id(row.get::<_, Vec<u8>>(1)?),
                    signing_pk: blob_to_key32(row.get::<_, Vec<u8>>(2)?),
                    exchange_pk: blob_to_key32(row.get::<_, Vec<u8>>(3)?),
                    display_name: row.get(4)?,
                    role: MemberRole::from_str(&role_str).unwrap_or(MemberRole::Member),
                    added_at: row.get(6)?,
                    added_by: blob_to_peer_id(row.get::<_, Vec<u8>>(7)?),
                    is_removed: row.get::<_, i32>(8)? != 0,
                })
            })
            .map_err(|e| CoreError::Store(format!("failed to query chat members: {e}")))?;

        let mut members = Vec::new();
        for row in rows {
            members.push(
                row.map_err(|e| CoreError::Store(format!("failed to read member row: {e}")))?,
            );
        }
        Ok(members)
    }

    pub fn remove_chat_member(
        &self,
        chat_id: &ChatId,
        peer_id: &PeerId,
    ) -> Result<(), CoreError> {
        let affected = self
            .connection()
            .execute(
                "UPDATE chat_members SET is_removed = 1 WHERE chat_id = ?1 AND peer_id = ?2",
                rusqlite::params![chat_id.as_slice(), peer_id.as_slice()],
            )
            .map_err(|e| CoreError::Store(format!("failed to remove chat member: {e}")))?;

        if affected == 0 {
            return Err(CoreError::NotFound(
                "chat member not found".to_string(),
            ));
        }
        Ok(())
    }

    pub fn insert_chat_key(&self, key: &ChatKey) -> Result<(), CoreError> {
        self.connection()
            .execute(
                "INSERT INTO chat_keys (chat_id, key_epoch, group_key_enc, created_at)
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![
                    key.chat_id.as_slice(),
                    key.key_epoch,
                    key.group_key_enc,
                    key.created_at,
                ],
            )
            .map_err(|e| CoreError::Store(format!("failed to insert chat key: {e}")))?;
        Ok(())
    }

    pub fn get_chat_key(
        &self,
        chat_id: &ChatId,
        epoch: u64,
    ) -> Result<Option<ChatKey>, CoreError> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT chat_id, key_epoch, group_key_enc, created_at
                 FROM chat_keys WHERE chat_id = ?1 AND key_epoch = ?2",
            )
            .map_err(|e| CoreError::Store(format!("failed to prepare get_chat_key: {e}")))?;

        let mut rows = statement
            .query_map(
                rusqlite::params![chat_id.as_slice(), epoch],
                |row| {
                    Ok(ChatKey {
                        chat_id: blob_to_chat_id(row.get::<_, Vec<u8>>(0)?),
                        key_epoch: row.get(1)?,
                        group_key_enc: row.get(2)?,
                        created_at: row.get(3)?,
                    })
                },
            )
            .map_err(|e| CoreError::Store(format!("failed to query chat key: {e}")))?;

        match rows.next() {
            Some(row) => Ok(Some(
                row.map_err(|e| CoreError::Store(format!("failed to read chat key row: {e}")))?,
            )),
            None => Ok(None),
        }
    }

    pub fn get_latest_chat_key(
        &self,
        chat_id: &ChatId,
    ) -> Result<Option<ChatKey>, CoreError> {
        let mut statement = self
            .connection()
            .prepare(
                "SELECT chat_id, key_epoch, group_key_enc, created_at
                 FROM chat_keys WHERE chat_id = ?1
                 ORDER BY key_epoch DESC LIMIT 1",
            )
            .map_err(|e| CoreError::Store(format!("failed to prepare get_latest_chat_key: {e}")))?;

        let mut rows = statement
            .query_map([chat_id.as_slice()], |row| {
                Ok(ChatKey {
                    chat_id: blob_to_chat_id(row.get::<_, Vec<u8>>(0)?),
                    key_epoch: row.get(1)?,
                    group_key_enc: row.get(2)?,
                    created_at: row.get(3)?,
                })
            })
            .map_err(|e| CoreError::Store(format!("failed to query latest chat key: {e}")))?;

        match rows.next() {
            Some(row) => Ok(Some(
                row.map_err(|e| {
                    CoreError::Store(format!("failed to read latest chat key row: {e}"))
                })?,
            )),
            None => Ok(None),
        }
    }
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

fn blob_to_key32(blob: Vec<u8>) -> [u8; 32] {
    let mut key = [0u8; 32];
    let len = blob.len().min(32);
    key[..len].copy_from_slice(&blob[..len]);
    key
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::db::Store;

    fn test_store() -> Store {
        Store::open_in_memory().unwrap()
    }

    fn sample_chat_id() -> ChatId {
        [1u8; 16]
    }

    fn sample_peer_id() -> PeerId {
        [2u8; 16]
    }

    fn sample_chat() -> Chat {
        Chat {
            chat_id: sample_chat_id(),
            chat_name: "test-chat".to_string(),
            owner_peer_id: sample_peer_id(),
            created_at: 1000,
            my_lamport_counter: 0,
        }
    }

    fn sample_member(peer_id: PeerId, role: MemberRole) -> ChatMember {
        ChatMember {
            chat_id: sample_chat_id(),
            peer_id,
            signing_pk: [10u8; 32],
            exchange_pk: [11u8; 32],
            display_name: "member".to_string(),
            role,
            added_at: 2000,
            added_by: sample_peer_id(),
            is_removed: false,
        }
    }

    // --- Chat CRUD ---

    #[test]
    fn insert_and_get_chat() {
        let store = test_store();
        let chat = sample_chat();

        store.insert_chat(&chat).unwrap();
        let loaded = store.get_chat(&chat.chat_id).unwrap().unwrap();

        assert_eq!(loaded.chat_id, chat.chat_id);
        assert_eq!(loaded.chat_name, "test-chat");
        assert_eq!(loaded.owner_peer_id, chat.owner_peer_id);
        assert_eq!(loaded.created_at, 1000);
        assert_eq!(loaded.my_lamport_counter, 0);
    }

    #[test]
    fn get_chat_returns_none_for_missing() {
        let store = test_store();
        let result = store.get_chat(&[99u8; 16]).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn list_chats_returns_all() {
        let store = test_store();

        let chat_a = Chat {
            chat_id: [1u8; 16],
            chat_name: "alpha".to_string(),
            owner_peer_id: sample_peer_id(),
            created_at: 1000,
            my_lamport_counter: 0,
        };
        let chat_b = Chat {
            chat_id: [2u8; 16],
            chat_name: "beta".to_string(),
            owner_peer_id: sample_peer_id(),
            created_at: 2000,
            my_lamport_counter: 5,
        };

        store.insert_chat(&chat_a).unwrap();
        store.insert_chat(&chat_b).unwrap();

        let chats = store.list_chats().unwrap();
        assert_eq!(chats.len(), 2);
        // Ordered by created_at DESC
        assert_eq!(chats[0].chat_name, "beta");
        assert_eq!(chats[1].chat_name, "alpha");
    }

    #[test]
    fn list_chats_empty() {
        let store = test_store();
        let chats = store.list_chats().unwrap();
        assert!(chats.is_empty());
    }

    #[test]
    fn insert_duplicate_chat_fails() {
        let store = test_store();
        let chat = sample_chat();

        store.insert_chat(&chat).unwrap();
        let result = store.insert_chat(&chat);
        assert!(result.is_err());
    }

    // --- ChatMember CRUD ---

    #[test]
    fn insert_and_get_chat_members() {
        let store = test_store();
        store.insert_chat(&sample_chat()).unwrap();

        let owner = sample_member(sample_peer_id(), MemberRole::Owner);
        let member_peer = [3u8; 16];
        let member = sample_member(member_peer, MemberRole::Member);

        store.insert_chat_member(&owner).unwrap();
        store.insert_chat_member(&member).unwrap();

        let members = store.get_chat_members(&sample_chat_id()).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].role, MemberRole::Owner);
        assert_eq!(members[1].role, MemberRole::Member);
    }

    #[test]
    fn remove_chat_member_sets_is_removed() {
        let store = test_store();
        store.insert_chat(&sample_chat()).unwrap();

        let member = sample_member([3u8; 16], MemberRole::Member);
        store.insert_chat_member(&member).unwrap();

        store
            .remove_chat_member(&sample_chat_id(), &[3u8; 16])
            .unwrap();

        let members = store.get_chat_members(&sample_chat_id()).unwrap();
        assert_eq!(members.len(), 1);
        assert!(members[0].is_removed);
    }

    #[test]
    fn remove_nonexistent_member_returns_not_found() {
        let store = test_store();
        let result = store.remove_chat_member(&sample_chat_id(), &[99u8; 16]);
        assert!(matches!(result, Err(CoreError::NotFound(_))));
    }

    #[test]
    fn chat_member_preserves_keys() {
        let store = test_store();
        store.insert_chat(&sample_chat()).unwrap();

        let member = ChatMember {
            chat_id: sample_chat_id(),
            peer_id: [5u8; 16],
            signing_pk: [20u8; 32],
            exchange_pk: [21u8; 32],
            display_name: "key-test".to_string(),
            role: MemberRole::Admin,
            added_at: 3000,
            added_by: sample_peer_id(),
            is_removed: false,
        };
        store.insert_chat_member(&member).unwrap();

        let loaded = store.get_chat_members(&sample_chat_id()).unwrap();
        assert_eq!(loaded[0].signing_pk, [20u8; 32]);
        assert_eq!(loaded[0].exchange_pk, [21u8; 32]);
        assert_eq!(loaded[0].display_name, "key-test");
        assert_eq!(loaded[0].role, MemberRole::Admin);
    }

    // --- ChatKey CRUD ---

    #[test]
    fn insert_and_get_chat_key() {
        let store = test_store();
        store.insert_chat(&sample_chat()).unwrap();

        let key = ChatKey {
            chat_id: sample_chat_id(),
            key_epoch: 0,
            group_key_enc: vec![1, 2, 3, 4],
            created_at: 1000,
        };

        store.insert_chat_key(&key).unwrap();
        let loaded = store.get_chat_key(&sample_chat_id(), 0).unwrap().unwrap();

        assert_eq!(loaded.chat_id, sample_chat_id());
        assert_eq!(loaded.key_epoch, 0);
        assert_eq!(loaded.group_key_enc, vec![1, 2, 3, 4]);
        assert_eq!(loaded.created_at, 1000);
    }

    #[test]
    fn get_chat_key_returns_none_for_missing_epoch() {
        let store = test_store();
        let result = store.get_chat_key(&sample_chat_id(), 99).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn get_latest_chat_key_returns_highest_epoch() {
        let store = test_store();
        store.insert_chat(&sample_chat()).unwrap();

        for epoch in 0..3 {
            let key = ChatKey {
                chat_id: sample_chat_id(),
                key_epoch: epoch,
                group_key_enc: vec![epoch as u8],
                created_at: 1000 + epoch,
            };
            store.insert_chat_key(&key).unwrap();
        }

        let latest = store.get_latest_chat_key(&sample_chat_id()).unwrap().unwrap();
        assert_eq!(latest.key_epoch, 2);
        assert_eq!(latest.group_key_enc, vec![2]);
    }

    #[test]
    fn get_latest_chat_key_returns_none_when_empty() {
        let store = test_store();
        let result = store.get_latest_chat_key(&sample_chat_id()).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn insert_duplicate_chat_key_fails() {
        let store = test_store();
        store.insert_chat(&sample_chat()).unwrap();

        let key = ChatKey {
            chat_id: sample_chat_id(),
            key_epoch: 0,
            group_key_enc: vec![1],
            created_at: 1000,
        };
        store.insert_chat_key(&key).unwrap();
        let result = store.insert_chat_key(&key);
        assert!(result.is_err());
    }
}

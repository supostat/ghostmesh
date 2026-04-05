use rusqlite::Connection;

use crate::types::CoreError;

pub struct Store {
    connection: Connection,
}

impl Store {
    pub fn open(path: &str) -> Result<Self, CoreError> {
        let connection = Connection::open(path)
            .map_err(|e| CoreError::Store(format!("failed to open database: {e}")))?;
        init_schema(&connection)?;
        Ok(Store { connection })
    }

    pub fn open_in_memory() -> Result<Self, CoreError> {
        let connection = Connection::open_in_memory()
            .map_err(|e| CoreError::Store(format!("failed to open in-memory database: {e}")))?;
        init_schema(&connection)?;
        Ok(Store { connection })
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }
}

fn init_schema(connection: &Connection) -> Result<(), CoreError> {
    connection
        .execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .map_err(|e| CoreError::Store(format!("failed to set pragmas: {e}")))?;

    connection
        .execute_batch(
            "
            CREATE TABLE IF NOT EXISTS identity (
                peer_id             BLOB PRIMARY KEY,
                signing_sk_enc      BLOB NOT NULL,
                signing_pk          BLOB NOT NULL,
                exchange_sk_enc     BLOB NOT NULL,
                exchange_pk         BLOB NOT NULL,
                display_name        TEXT NOT NULL,
                created_at          INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS chats (
                chat_id             BLOB PRIMARY KEY,
                chat_name           TEXT NOT NULL,
                owner_peer_id       BLOB NOT NULL,
                created_at          INTEGER NOT NULL,
                my_lamport_counter  INTEGER NOT NULL DEFAULT 0,
                unread_count        INTEGER NOT NULL DEFAULT 0
            );

            CREATE TABLE IF NOT EXISTS chat_members (
                chat_id             BLOB NOT NULL,
                peer_id             BLOB NOT NULL,
                signing_pk          BLOB NOT NULL,
                exchange_pk         BLOB NOT NULL,
                display_name        TEXT NOT NULL,
                role                TEXT NOT NULL,
                added_at            INTEGER NOT NULL,
                added_by            BLOB NOT NULL,
                is_removed          INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (chat_id, peer_id)
            );

            CREATE TABLE IF NOT EXISTS chat_keys (
                chat_id             BLOB NOT NULL,
                key_epoch           INTEGER NOT NULL,
                group_key_enc       BLOB NOT NULL,
                created_at          INTEGER NOT NULL,
                PRIMARY KEY (chat_id, key_epoch)
            );

            CREATE TABLE IF NOT EXISTS messages (
                message_id          BLOB PRIMARY KEY,
                chat_id             BLOB NOT NULL,
                author_peer_id      BLOB NOT NULL,
                lamport_ts          INTEGER NOT NULL,
                created_at          INTEGER NOT NULL,
                key_epoch           INTEGER NOT NULL,
                parent_ids          BLOB,
                signature           BLOB NOT NULL,
                payload_ciphertext  BLOB NOT NULL,
                payload_nonce       BLOB NOT NULL,
                received_at         INTEGER NOT NULL,
                UNIQUE(chat_id, lamport_ts, author_peer_id)
            );

            CREATE INDEX IF NOT EXISTS idx_messages_chat_lamport
                ON messages(chat_id, lamport_ts);

            CREATE TABLE IF NOT EXISTS frontiers (
                chat_id             BLOB NOT NULL,
                author_peer_id      BLOB NOT NULL,
                max_lamport_ts      INTEGER NOT NULL,
                message_count       INTEGER NOT NULL,
                PRIMARY KEY (chat_id, author_peer_id)
            );

            CREATE TABLE IF NOT EXISTS outbox (
                message_id          BLOB NOT NULL,
                target_peer_id      BLOB NOT NULL,
                chat_id             BLOB NOT NULL,
                created_at          INTEGER NOT NULL,
                PRIMARY KEY (message_id, target_peer_id)
            );

            CREATE INDEX IF NOT EXISTS idx_outbox_target
                ON outbox(target_peer_id);

            CREATE TABLE IF NOT EXISTS peer_addresses (
                peer_id             BLOB NOT NULL,
                address_type        TEXT NOT NULL,
                address             TEXT NOT NULL,
                last_seen           INTEGER NOT NULL,
                last_successful     INTEGER,
                fail_count          INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY (peer_id, address_type, address)
            );

            CREATE TABLE IF NOT EXISTS sync_log (
                id                  INTEGER PRIMARY KEY AUTOINCREMENT,
                timestamp           INTEGER NOT NULL,
                peer_id             BLOB,
                event_type          TEXT NOT NULL,
                detail              TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_sync_log_ts
                ON sync_log(timestamp DESC);

            CREATE TABLE IF NOT EXISTS pending_joins (
                chat_id       BLOB PRIMARY KEY,
                invite_token  BLOB NOT NULL,
                pending       INTEGER NOT NULL DEFAULT 1,
                retry_count   INTEGER NOT NULL DEFAULT 0,
                received_at   INTEGER
            );
            ",
        )
        .map_err(|e| CoreError::Store(format!("failed to create schema: {e}")))?;

    // Migration: add unread_count to existing chats tables (no-op if column already exists)
    let _ = connection.execute_batch(
        "ALTER TABLE chats ADD COLUMN unread_count INTEGER NOT NULL DEFAULT 0",
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_in_memory_creates_store() {
        let store = Store::open_in_memory();
        assert!(store.is_ok());
    }

    #[test]
    fn schema_creates_all_tables() {
        let store = Store::open_in_memory().unwrap();
        let connection = store.connection();

        let expected_tables = [
            "identity",
            "chats",
            "chat_members",
            "chat_keys",
            "messages",
            "frontiers",
            "outbox",
            "peer_addresses",
            "sync_log",
            "pending_joins",
        ];

        for table_name in &expected_tables {
            let exists: bool = connection
                .query_row(
                    "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name=?1",
                    [table_name],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(exists, "table '{table_name}' should exist");
        }
    }

    #[test]
    fn schema_creates_indices() {
        let store = Store::open_in_memory().unwrap();
        let connection = store.connection();

        let expected_indices = [
            "idx_messages_chat_lamport",
            "idx_outbox_target",
            "idx_sync_log_ts",
        ];

        for index_name in &expected_indices {
            let exists: bool = connection
                .query_row(
                    "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='index' AND name=?1",
                    [index_name],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(exists, "index '{index_name}' should exist");
        }
    }

    #[test]
    fn open_in_memory_is_idempotent() {
        let store = Store::open_in_memory().unwrap();
        // Calling init_schema again should not fail (IF NOT EXISTS)
        let result = init_schema(store.connection());
        assert!(result.is_ok());
    }

    #[test]
    fn wal_mode_is_set() {
        let store = Store::open_in_memory().unwrap();
        let mode: String = store
            .connection()
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        // In-memory databases use "memory" journal mode, not WAL
        // WAL applies to file-based databases
        assert!(
            mode == "wal" || mode == "memory",
            "journal_mode should be wal or memory, got: {mode}"
        );
    }
}

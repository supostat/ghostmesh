use serde::{Deserialize, Serialize};

// --- IPC DTOs (Commands → Frontend) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityInfo {
    pub peer_id: String,
    pub display_name: String,
    pub fingerprint: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatInfo {
    pub chat_id: String,
    pub chat_name: String,
    pub owner_peer_id: String,
    pub created_at: u64,
    pub member_count: u32,
    pub online_count: u32,
    pub last_message_preview: Option<String>,
    pub last_message_at: Option<u64>,
    pub unread_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatDetail {
    pub chat_id: String,
    pub chat_name: String,
    pub owner_peer_id: String,
    pub created_at: u64,
    pub members: Vec<MemberInfo>,
    pub key_epoch: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberInfo {
    pub peer_id: String,
    pub display_name: String,
    pub fingerprint: String,
    pub role: String,
    pub is_online: bool,
    pub added_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageInfo {
    pub message_id: String,
    pub chat_id: String,
    pub author_peer_id: String,
    pub author_name: String,
    pub lamport_ts: u64,
    pub created_at: u64,
    pub text: String,
    pub delivery_status: DeliveryStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeliveryStatus {
    Queued,
    Partial { delivered: u32, total: u32 },
    All,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessagePacket {
    pub message_id: String,
    pub chat_id: String,
    pub author_peer_id: String,
    pub lamport_ts: u64,
    pub created_at: u64,
    pub key_epoch: u64,
    pub parent_ids: Vec<String>,
    pub signature: String,
    pub payload_size: usize,
    pub text: String,
    pub delivery_acks: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InviteCode {
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerInfo {
    pub peer_id: String,
    pub display_name: String,
    pub addresses: Vec<String>,
    pub last_seen: Option<u64>,
    pub is_connected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionInfo {
    pub peer_id: String,
    pub display_name: String,
    pub address: String,
    pub connected_at: u64,
    pub messages_synced: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxInfo {
    pub message_id: String,
    pub target_peer_id: String,
    pub chat_id: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncLogInfo {
    pub id: i64,
    pub timestamp: u64,
    pub peer_id: Option<String>,
    pub event_type: String,
    pub detail: Option<String>,
}

// --- IPC Events (Rust → Frontend) ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerEvent {
    pub peer_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncProgress {
    pub chat_id: String,
    pub received: u64,
    pub total: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncComplete {
    pub chat_id: String,
    pub new_messages: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryAck {
    pub message_id: String,
    pub peer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStatus {
    pub connected_peers: u32,
    pub outbox_size: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemberEvent {
    pub chat_id: String,
    pub peer_id: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyRotationEvent {
    pub chat_id: String,
    pub new_key_epoch: u64,
}

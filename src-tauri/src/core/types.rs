use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

// --- Primitives ---

pub type PeerId = [u8; 16];
pub type MessageId = [u8; 32];
pub type ChatId = [u8; 16];
pub type GroupKey = [u8; 32];
pub type InviteToken = [u8; 32];

// --- Identity ---

#[derive(Clone)]
pub struct Identity {
    pub peer_id: PeerId,
    pub signing_keypair: SigningKeypair,
    pub exchange_keypair: ExchangeKeypair,
    pub display_name: String,
    pub created_at: u64,
}

#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct SigningKeypair {
    pub secret: [u8; 64],
    pub public: [u8; 32],
}

#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct ExchangeKeypair {
    pub secret: [u8; 32],
    pub public: [u8; 32],
}

// --- Chat ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chat {
    pub chat_id: ChatId,
    pub chat_name: String,
    pub owner_peer_id: PeerId,
    pub created_at: u64,
    pub my_lamport_counter: u64,
    pub unread_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMember {
    pub chat_id: ChatId,
    pub peer_id: PeerId,
    pub signing_pk: [u8; 32],
    pub exchange_pk: [u8; 32],
    pub display_name: String,
    pub role: MemberRole,
    pub added_at: u64,
    pub added_by: PeerId,
    pub is_removed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MemberRole {
    Owner,
    Admin,
    Member,
}

impl MemberRole {
    pub fn as_str(&self) -> &'static str {
        match self {
            MemberRole::Owner => "owner",
            MemberRole::Admin => "admin",
            MemberRole::Member => "member",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "owner" => Some(MemberRole::Owner),
            "admin" => Some(MemberRole::Admin),
            "member" => Some(MemberRole::Member),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatKey {
    pub chat_id: ChatId,
    pub key_epoch: u64,
    pub group_key_enc: Vec<u8>,
    pub created_at: u64,
}

// --- Message ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub message_id: MessageId,
    pub chat_id: ChatId,
    pub author_peer_id: PeerId,
    pub lamport_ts: u64,
    pub created_at: u64,
    pub key_epoch: u64,
    pub parent_ids: Vec<MessageId>,
    pub signature: Vec<u8>,
    pub payload_ciphertext: Vec<u8>,
    pub payload_nonce: [u8; 24],
    pub received_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessagePayload {
    Text { body: String },
    SystemEvent(SystemEvent),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SystemEvent {
    MemberAdded {
        member: ChatMember,
        encrypted_group_key: Vec<u8>,
    },
    MemberRemoved {
        peer_id: PeerId,
        new_key_epoch: u64,
        rekey_packages: Vec<RekeyPackage>,
    },
    ChatRenamed {
        new_name: String,
    },
    KeyRotation {
        new_key_epoch: u64,
        rekey_packages: Vec<RekeyPackage>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RekeyPackage {
    pub target_peer_id: PeerId,
    pub encrypted_key: Vec<u8>,
}

// --- Sync ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontierEntry {
    pub author_peer_id: PeerId,
    pub max_lamport_ts: u64,
    pub message_count: u64,
}

// --- Network ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerAddress {
    pub peer_id: PeerId,
    pub address_type: String,
    pub address: String,
    pub last_seen: u64,
    pub last_successful: Option<u64>,
    pub fail_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncLogEntry {
    pub id: i64,
    pub timestamp: u64,
    pub peer_id: Option<PeerId>,
    pub event_type: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboxEntry {
    pub message_id: MessageId,
    pub target_peer_id: PeerId,
    pub chat_id: ChatId,
    pub created_at: u64,
}

// --- Wire Protocol ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WireMessage {
    SyncRequest {
        chat_id: ChatId,
        frontier: Vec<FrontierEntry>,
    },
    SyncResponse {
        chat_id: ChatId,
        messages: Vec<Message>,
        frontier: Vec<FrontierEntry>,
    },
    SyncAck {
        chat_id: ChatId,
        received: Vec<MessageId>,
    },
    JoinRequest {
        chat_id: ChatId,
        invite_token: InviteToken,
        identity: PeerIdentityPacket,
    },
    JoinResponse {
        accepted: bool,
        group_key_enc: Option<Vec<u8>>,
        members: Vec<ChatMember>,
        recent_messages: Vec<Message>,
    },
    PeerExchange {
        chat_id: ChatId,
        peers: Vec<PeerAddress>,
    },
    Ping {
        timestamp: u64,
    },
    Pong {
        timestamp: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerIdentityPacket {
    pub peer_id: PeerId,
    pub signing_pk: [u8; 32],
    pub exchange_pk: [u8; 32],
    pub display_name: String,
}

// --- Pending Join ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JoinPending {
    pub chat_id: ChatId,
    pub invite_token: InviteToken,
    pub pending: bool,
    pub retry_count: u32,
    pub received_at: u64,
}

// --- Invite ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatInvite {
    pub chat_id: ChatId,
    pub chat_name: String,
    pub owner_peer_id: PeerId,
    pub owner_signing_pk: [u8; 32],
    pub owner_exchange_pk: [u8; 32],
    pub owner_addresses: Vec<PeerAddress>,
    pub invite_token: InviteToken,
    pub created_at: u64,
}

// --- Settings ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub display_name: String,
    pub listen_port: u16,
    pub mdns_enabled: bool,
    pub message_ttl_days: Option<u32>,
    pub theme: String,
    pub auto_update_enabled: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            display_name: String::new(),
            listen_port: 9473,
            mdns_enabled: true,
            message_ttl_days: None,
            theme: "default".to_string(),
            auto_update_enabled: true,
        }
    }
}

// --- Errors ---

#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("crypto error: {0}")]
    Crypto(String),
    #[error("store error: {0}")]
    Store(String),
    #[error("net error: {0}")]
    Net(String),
    #[error("sync error: {0}")]
    Sync(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("invalid input: {0}")]
    InvalidInput(String),
    #[error("identity not initialized")]
    IdentityNotInitialized,
}

impl From<CoreError> for String {
    fn from(e: CoreError) -> String {
        e.to_string()
    }
}

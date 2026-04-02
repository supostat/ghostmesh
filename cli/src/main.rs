use std::path::PathBuf;

use clap::{Parser, Subcommand};

use ghostmesh_core::crypto::encrypt::{decrypt_key_storage, decrypt_message, encrypt_key_storage};
use ghostmesh_core::crypto::identity::{derive_peer_id, generate_identity};
use ghostmesh_core::crypto::sign::sign;
use ghostmesh_core::store::Store;
use ghostmesh_core::sync::engine::SyncEngine;
use ghostmesh_core::sync::lamport::LamportClock;
use ghostmesh_core::types::{
    Chat, ChatKey, ChatMember, CoreError, GroupKey, MemberRole, Message, MessagePayload,
    OutboxEntry,
};

#[derive(Parser)]
#[command(name = "ghostmesh-cli", version, about = "GhostMesh P2P Messenger — Dev CLI")]
struct Cli {
    /// Path to the database file
    #[arg(short, long, default_value = "ghostmesh-cli.db")]
    database: PathBuf,

    /// Password for key encryption/decryption
    #[arg(short, long, env = "GHOSTMESH_PASSWORD")]
    password: String,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Create or show current identity
    Identity {
        /// Display name for new identity
        #[arg(short, long)]
        name: Option<String>,
    },
    /// Send a text message to a chat
    Send {
        /// Chat ID (hex)
        #[arg(short, long)]
        chat: String,
        /// Message text
        text: String,
    },
    /// Read messages from a chat
    Read {
        /// Chat ID (hex)
        #[arg(short, long)]
        chat: String,
        /// Number of messages to show
        #[arg(short, long, default_value = "20")]
        limit: u32,
    },
    /// List known peers
    Peers,
    /// Show sync status for a chat
    Sync {
        /// Chat ID (hex)
        #[arg(short, long)]
        chat: String,
    },
    /// List all chats
    Chats,
    /// Create a new chat
    CreateChat {
        /// Chat name
        name: String,
    },
}

fn main() {
    tracing_subscriber::fmt::init();
    let cli = Cli::parse();

    let db_path = cli
        .database
        .to_str()
        .expect("database path must be valid UTF-8");
    let store = Store::open(db_path).expect("failed to open database");

    match cli.command {
        Command::Identity { name } => cmd_identity(&store, &cli.password, name),
        Command::Send { chat, text } => cmd_send(&store, &cli.password, &chat, &text),
        Command::Read { chat, limit } => cmd_read(&store, &cli.password, &chat, limit),
        Command::Peers => cmd_peers(&store),
        Command::Sync { chat } => cmd_sync(&store, &chat),
        Command::Chats => cmd_chats(&store),
        Command::CreateChat { name } => cmd_create_chat(&store, &cli.password, &name),
    }
}

fn cmd_identity(store: &Store, password: &str, name: Option<String>) {
    match store.get_identity() {
        Ok(Some(stored)) => {
            println!("Identity exists:");
            println!("  peer_id:      {}", hex::encode(stored.peer_id));
            println!(
                "  fingerprint:  {}",
                hex::encode(&stored.signing_pk[..8])
            );
            println!("  display_name: {}", stored.display_name);
            println!("  created_at:   {}", stored.created_at);
        }
        Ok(None) => {
            let display_name = name.unwrap_or_else(|| {
                eprintln!("No identity found. Use --name to create one.");
                std::process::exit(1);
            });

            let identity = generate_identity(display_name);
            let signing_sk_enc = encrypt_key_storage(password, &identity.signing_keypair.secret)
                .expect("failed to encrypt signing key");
            let exchange_sk_enc =
                encrypt_key_storage(password, &identity.exchange_keypair.secret)
                    .expect("failed to encrypt exchange key");

            store
                .save_identity(
                    &identity.peer_id,
                    &signing_sk_enc,
                    &identity.signing_keypair.public,
                    &exchange_sk_enc,
                    &identity.exchange_keypair.public,
                    &identity.display_name,
                    identity.created_at,
                )
                .expect("failed to save identity");

            println!("Identity created:");
            println!("  peer_id:      {}", hex::encode(identity.peer_id));
            println!(
                "  fingerprint:  {}",
                hex::encode(&identity.signing_keypair.public[..8])
            );
            println!("  display_name: {}", identity.display_name);
        }
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    }
}

fn cmd_send(store: &Store, password: &str, chat_hex: &str, text: &str) {
    let chat_id = hex_to_chat_id(chat_hex);
    let stored = require_identity(store);

    let chat_key = store
        .get_latest_chat_key(&chat_id)
        .expect("failed to get chat key")
        .expect("no group key for chat");

    let group_key_bytes =
        decrypt_key_storage(password, &chat_key.group_key_enc).expect("failed to decrypt group key");
    let mut group_key = [0u8; 32];
    group_key.copy_from_slice(&group_key_bytes);

    let payload = MessagePayload::Text {
        body: text.to_string(),
    };
    let mut payload_cbor = Vec::new();
    ciborium::into_writer(&payload, &mut payload_cbor).expect("failed to encode payload");

    let (ciphertext, nonce) =
        ghostmesh_core::crypto::encrypt::encrypt_message(&group_key, &payload_cbor)
            .expect("failed to encrypt");

    let signing_sk_bytes =
        decrypt_key_storage(password, &stored.signing_sk_enc).expect("failed to decrypt signing key");
    let mut signing_sk = [0u8; 64];
    signing_sk.copy_from_slice(&signing_sk_bytes);

    let mut lamport = LamportClock::new();
    let lamport_ts = lamport.on_send();
    let created_at = now_secs();
    let message_id: [u8; 32] = rand::random();

    let mut signable = Vec::new();
    signable.extend_from_slice(&message_id);
    signable.extend_from_slice(&chat_id);
    signable.extend_from_slice(&stored.peer_id);
    signable.extend_from_slice(&lamport_ts.to_le_bytes());
    signable.extend_from_slice(&ciphertext);
    signable.extend_from_slice(&nonce);

    let signature = sign(&signing_sk, &signable).expect("failed to sign");

    let message = Message {
        message_id,
        chat_id,
        author_peer_id: stored.peer_id,
        lamport_ts,
        created_at,
        key_epoch: chat_key.key_epoch,
        parent_ids: Vec::new(),
        signature,
        payload_ciphertext: ciphertext,
        payload_nonce: nonce,
        received_at: created_at,
    };

    store.insert_message(&message).expect("failed to insert message");

    let members = store
        .get_chat_members(&chat_id)
        .expect("failed to get members");
    for member in &members {
        if member.peer_id != stored.peer_id && !member.is_removed {
            store
                .insert_outbox_entry(&OutboxEntry {
                    message_id,
                    target_peer_id: member.peer_id,
                    chat_id,
                    created_at,
                })
                .expect("failed to insert outbox entry");
        }
    }

    println!(
        "Sent: L={} id={}",
        lamport_ts,
        &hex::encode(message_id)[..16]
    );
}

fn cmd_read(store: &Store, password: &str, chat_hex: &str, limit: u32) {
    let chat_id = hex_to_chat_id(chat_hex);

    let messages = store
        .get_messages(&chat_id, None, limit)
        .expect("failed to get messages");

    if messages.is_empty() {
        println!("No messages.");
        return;
    }

    let chat_key = store
        .get_latest_chat_key(&chat_id)
        .expect("failed to get chat key");

    for msg in &messages {
        let text = if let Some(ref key) = chat_key {
            match decrypt_key_storage(password, &key.group_key_enc) {
                Ok(gk_bytes) => {
                    let mut gk = [0u8; 32];
                    gk.copy_from_slice(&gk_bytes);
                    match decrypt_message(&gk, &msg.payload_nonce, &msg.payload_ciphertext) {
                        Ok(plaintext) => {
                            match ciborium::from_reader::<MessagePayload, _>(plaintext.as_slice()) {
                                Ok(MessagePayload::Text { body }) => body,
                                _ => "<decode error>".to_string(),
                            }
                        }
                        Err(_) => "<decrypt error>".to_string(),
                    }
                }
                Err(_) => "<key decrypt error>".to_string(),
            }
        } else {
            "<no key>".to_string()
        };

        let author_short = &hex::encode(msg.author_peer_id)[..8];
        println!(
            "  [L={}] {}: {}",
            msg.lamport_ts, author_short, text
        );
    }
}

fn cmd_peers(store: &Store) {
    let addresses = store
        .get_all_peer_addresses()
        .expect("failed to get peer addresses");

    if addresses.is_empty() {
        println!("No known peers.");
        return;
    }

    println!("Known peers:");
    for addr in &addresses {
        println!(
            "  {} ({}) — {} [fails: {}, seen: {}]",
            &hex::encode(addr.peer_id)[..8],
            addr.address_type,
            addr.address,
            addr.fail_count,
            addr.last_seen
        );
    }
}

fn cmd_sync(store: &Store, chat_hex: &str) {
    let chat_id = hex_to_chat_id(chat_hex);

    let frontier = store
        .get_frontier(&chat_id)
        .expect("failed to get frontier");

    if frontier.is_empty() {
        println!("Empty frontier — no messages synced yet.");
        return;
    }

    println!("Frontier for chat {}:", &chat_hex[..8.min(chat_hex.len())]);
    for entry in &frontier {
        println!(
            "  author={} max_lamport={} messages={}",
            &hex::encode(entry.author_peer_id)[..8],
            entry.max_lamport_ts,
            entry.message_count
        );
    }

    let outbox = store
        .get_outbox_for_chat(&chat_id)
        .expect("failed to get outbox");
    println!("Outbox: {} pending deliveries", outbox.len());
}

fn cmd_chats(store: &Store) {
    let chats = store.list_chats().expect("failed to list chats");

    if chats.is_empty() {
        println!("No chats.");
        return;
    }

    println!("Chats:");
    for chat in &chats {
        let members = store
            .get_chat_members(&chat.chat_id)
            .unwrap_or_default();
        let active = members.iter().filter(|m| !m.is_removed).count();
        println!(
            "  {} — \"{}\" [{} members]",
            &hex::encode(chat.chat_id)[..8],
            chat.chat_name,
            active
        );
    }
}

fn cmd_create_chat(store: &Store, password: &str, name: &str) {
    let stored = require_identity(store);

    let chat_id: [u8; 16] = uuid::Uuid::new_v4().into_bytes();
    let created_at = now_secs();

    let chat = Chat {
        chat_id,
        chat_name: name.to_string(),
        owner_peer_id: stored.peer_id,
        created_at,
        my_lamport_counter: 0,
    };
    store.insert_chat(&chat).expect("failed to insert chat");

    let group_key: GroupKey = rand::random();
    let group_key_enc =
        encrypt_key_storage(password, &group_key).expect("failed to encrypt group key");

    let chat_key = ChatKey {
        chat_id,
        key_epoch: 0,
        group_key_enc,
        created_at,
    };
    store
        .insert_chat_key(&chat_key)
        .expect("failed to insert chat key");

    let owner = ChatMember {
        chat_id,
        peer_id: stored.peer_id,
        signing_pk: stored.signing_pk,
        exchange_pk: stored.exchange_pk,
        display_name: stored.display_name,
        role: MemberRole::Owner,
        added_at: created_at,
        added_by: stored.peer_id,
        is_removed: false,
    };
    store
        .insert_chat_member(&owner)
        .expect("failed to insert owner");

    println!("Chat created: {} — \"{}\"", hex::encode(chat_id), name);
}

fn require_identity(store: &Store) -> ghostmesh_core::store::StoredIdentity {
    store
        .get_identity()
        .expect("failed to get identity")
        .unwrap_or_else(|| {
            eprintln!("No identity. Run: ghostmesh-cli identity --name <name>");
            std::process::exit(1);
        })
}

fn hex_to_chat_id(hex_str: &str) -> [u8; 16] {
    let bytes = hex::decode(hex_str).expect("invalid chat_id hex");
    assert!(bytes.len() == 16, "chat_id must be 16 bytes (32 hex chars)");
    let mut id = [0u8; 16];
    id.copy_from_slice(&bytes);
    id
}

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs()
}

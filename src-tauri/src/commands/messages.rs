use tauri::{AppHandle, State};

use ghostmesh_core::crypto::encrypt::{decrypt_key_storage, decrypt_message, encrypt_message};
use ghostmesh_core::crypto::sign::sign;
use ghostmesh_core::sync::SyncEngine;
use ghostmesh_core::types::{CoreError, Message, MessagePayload, OutboxEntry, SystemEvent};

use crate::events::emit_message_new;
use crate::state::AppState;
use crate::types::{DeliveryStatus, MessageInfo, MessagePacket};

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs()
}

#[tauri::command]
pub async fn send_message(
    app: AppHandle,
    state: State<'_, AppState>,
    chat_id: String,
    text: String,
    password: String,
) -> Result<MessageInfo, String> {
    let chat_id_bytes = hex_to_chat_id(&chat_id)?;

    let store = state.store.lock().map_err(|e| e.to_string())?;
    let mut lamport = state.lamport.lock().map_err(|e| e.to_string())?;

    let stored_identity = store
        .get_identity()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| CoreError::IdentityNotInitialized.to_string())?;

    // Check if chat is pending key exchange
    let pending = store
        .get_pending_join(&chat_id_bytes)
        .map_err(|e| e.to_string())?;
    if pending.map(|pj| pj.pending).unwrap_or(false) {
        return Err(
            "chat is pending key exchange — waiting for owner to share the group key".to_string(),
        );
    }

    // Get the latest group key and decrypt it
    let chat_key = store
        .get_latest_chat_key(&chat_id_bytes)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| CoreError::NotFound("no group key for chat".to_string()).to_string())?;

    let group_key_bytes =
        decrypt_key_storage(&password, &chat_key.group_key_enc).map_err(|e| e.to_string())?;
    if group_key_bytes.len() != 32 {
        return Err("invalid group key length after decryption".to_string());
    }
    let mut group_key = [0u8; 32];
    group_key.copy_from_slice(&group_key_bytes);

    // Encode payload as CBOR
    let payload = MessagePayload::Text { body: text.clone() };
    let mut payload_cbor = Vec::new();
    ciborium::into_writer(&payload, &mut payload_cbor)
        .map_err(|e| format!("failed to encode payload: {e}"))?;

    // Encrypt payload
    let (ciphertext, nonce) =
        encrypt_message(&group_key, &payload_cbor).map_err(|e| e.to_string())?;

    // Decrypt signing key to sign the message
    let signing_sk_bytes = decrypt_key_storage(&password, &stored_identity.signing_sk_enc)
        .map_err(|e| e.to_string())?;
    if signing_sk_bytes.len() != 64 {
        return Err("invalid signing key length after decryption".to_string());
    }
    let mut signing_sk = [0u8; 64];
    signing_sk.copy_from_slice(&signing_sk_bytes);

    // Build signable data: header || ciphertext || nonce
    let lamport_ts = lamport.on_send();
    let created_at = now_secs();
    let message_id: [u8; 32] = rand::random();

    let mut signable = Vec::new();
    signable.extend_from_slice(&message_id);
    signable.extend_from_slice(&chat_id_bytes);
    signable.extend_from_slice(&stored_identity.peer_id);
    signable.extend_from_slice(&lamport_ts.to_le_bytes());
    signable.extend_from_slice(&ciphertext);
    signable.extend_from_slice(&nonce);

    let signature = sign(&signing_sk, &signable).map_err(|e| e.to_string())?;

    let message = Message {
        message_id,
        chat_id: chat_id_bytes,
        author_peer_id: stored_identity.peer_id,
        lamport_ts,
        created_at,
        key_epoch: chat_key.key_epoch,
        parent_ids: Vec::new(),
        signature,
        payload_ciphertext: ciphertext,
        payload_nonce: nonce,
        received_at: created_at,
    };

    store
        .insert_message(&message)
        .map_err(|e| e.to_string())?;

    // Add to outbox for all other chat members
    let members = store
        .get_chat_members(&chat_id_bytes)
        .map_err(|e| e.to_string())?;
    for member in &members {
        if member.peer_id != stored_identity.peer_id && !member.is_removed {
            let entry = OutboxEntry {
                message_id,
                target_peer_id: member.peer_id,
                chat_id: chat_id_bytes,
                created_at,
            };
            store
                .insert_outbox_entry(&entry)
                .map_err(|e| e.to_string())?;
        }
    }

    let message_info = MessageInfo {
        message_id: hex::encode(message_id),
        chat_id: chat_id.clone(),
        author_peer_id: hex::encode(stored_identity.peer_id),
        author_name: stored_identity.display_name,
        lamport_ts,
        created_at,
        text,
        delivery_status: DeliveryStatus::Queued,
    };

    let _ = emit_message_new(&app, &message_info);

    Ok(message_info)
}

#[tauri::command]
pub async fn get_messages(
    state: State<'_, AppState>,
    chat_id: String,
    password: String,
    before_lamport: Option<u64>,
    limit: Option<u32>,
) -> Result<Vec<MessageInfo>, String> {
    let chat_id_bytes = hex_to_chat_id(&chat_id)?;
    let message_limit = limit.unwrap_or(100);

    let store = state.store.lock().map_err(|e| e.to_string())?;

    let stored_identity = store
        .get_identity()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| CoreError::IdentityNotInitialized.to_string())?;

    let messages = store
        .get_messages(&chat_id_bytes, before_lamport, message_limit)
        .map_err(|e| e.to_string())?;

    store
        .reset_unread_count(&chat_id_bytes)
        .map_err(|e| e.to_string())?;

    let members = store
        .get_chat_members(&chat_id_bytes)
        .map_err(|e| e.to_string())?;

    let mut result = Vec::with_capacity(messages.len());

    for message in &messages {
        let text = decrypt_message_text(&store, &chat_id_bytes, message, &password);

        // Try to process rekey if this is a MemberRemoved system event
        try_process_rekey(
            &store,
            &chat_id_bytes,
            message,
            &password,
            &stored_identity,
        );

        let author_name = members
            .iter()
            .find(|m| m.peer_id == message.author_peer_id)
            .map(|m| m.display_name.clone())
            .unwrap_or_else(|| hex::encode(&message.author_peer_id[..4]));

        let delivery_status = if message.author_peer_id == stored_identity.peer_id {
            compute_delivery_status(&store, &message.message_id, &chat_id_bytes)?
        } else {
            DeliveryStatus::All
        };

        result.push(MessageInfo {
            message_id: hex::encode(message.message_id),
            chat_id: chat_id.clone(),
            author_peer_id: hex::encode(message.author_peer_id),
            author_name,
            lamport_ts: message.lamport_ts,
            created_at: message.created_at,
            text,
            delivery_status,
        });
    }

    Ok(result)
}

/// Attempts to decrypt a message and, if it contains a MemberRemoved
/// system event with a rekey package for the current user, processes
/// the rekey to store the new group key locally.
fn try_process_rekey(
    store: &ghostmesh_core::store::Store,
    chat_id: &[u8; 16],
    message: &Message,
    password: &str,
    stored_identity: &ghostmesh_core::store::StoredIdentity,
) {
    // Check if we already have the key for this epoch — skip if so
    if store.get_chat_key(chat_id, message.key_epoch).ok().flatten().is_some() {
        // We may already have processed this rekey
        // But we still need to check if the NEW epoch key is present
    }

    // Decrypt the message payload
    let chat_key = match store.get_chat_key(chat_id, message.key_epoch) {
        Ok(Some(key)) => key,
        _ => return,
    };

    let group_key_bytes = match decrypt_key_storage(password, &chat_key.group_key_enc) {
        Ok(bytes) if bytes.len() == 32 => bytes,
        _ => return,
    };

    let mut group_key = [0u8; 32];
    group_key.copy_from_slice(&group_key_bytes);

    let plaintext = match decrypt_message(&group_key, &message.payload_nonce, &message.payload_ciphertext) {
        Ok(data) => data,
        Err(_) => return,
    };

    let payload: MessagePayload = match ciborium::from_reader(plaintext.as_slice()) {
        Ok(p) => p,
        Err(_) => return,
    };

    let (new_key_epoch, rekey_packages) = match payload {
        MessagePayload::SystemEvent(SystemEvent::MemberRemoved {
            new_key_epoch,
            rekey_packages,
            ..
        }) => (new_key_epoch, rekey_packages),
        _ => return,
    };

    // Check if we already have the new epoch key
    if store.get_chat_key(chat_id, new_key_epoch).ok().flatten().is_some() {
        return;
    }

    // Find our rekey package
    let my_package = match rekey_packages.iter().find(|rp| rp.target_peer_id == stored_identity.peer_id) {
        Some(p) => p,
        None => return,
    };

    // Decrypt own exchange_sk
    let exchange_sk_bytes = match decrypt_key_storage(password, &stored_identity.exchange_sk_enc) {
        Ok(bytes) if bytes.len() == 32 => bytes,
        _ => return,
    };
    let mut exchange_sk = [0u8; 32];
    exchange_sk.copy_from_slice(&exchange_sk_bytes);

    // Get sender's exchange_pk
    let members = match store.get_chat_members(chat_id) {
        Ok(m) => m,
        Err(_) => return,
    };
    let sender = match members.iter().find(|m| m.peer_id == message.author_peer_id) {
        Some(s) => s,
        None => return,
    };

    let _ = SyncEngine::process_rekey_package(
        store,
        chat_id,
        new_key_epoch,
        my_package,
        &exchange_sk,
        &sender.exchange_pk,
        password,
    );
}

#[tauri::command]
pub async fn get_message_detail(
    state: State<'_, AppState>,
    message_id: String,
    password: String,
) -> Result<MessagePacket, String> {
    let message_id_bytes = hex_to_message_id(&message_id)?;

    let store = state.store.lock().map_err(|e| e.to_string())?;

    let message = store
        .get_message(&message_id_bytes)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| CoreError::NotFound("message not found".to_string()).to_string())?;

    let text = decrypt_message_text(&store, &message.chat_id, &message, &password);

    let parent_ids: Vec<String> = message.parent_ids.iter().map(hex::encode).collect();

    Ok(MessagePacket {
        message_id: hex::encode(message.message_id),
        chat_id: hex::encode(message.chat_id),
        author_peer_id: hex::encode(message.author_peer_id),
        lamport_ts: message.lamport_ts,
        created_at: message.created_at,
        key_epoch: message.key_epoch,
        parent_ids,
        signature: hex::encode(&message.signature),
        payload_size: message.payload_ciphertext.len(),
        text,
        delivery_acks: Vec::new(), // Populated from outbox absence in future
    })
}

fn decrypt_message_text(
    store: &ghostmesh_core::store::Store,
    chat_id: &[u8; 16],
    message: &Message,
    password: &str,
) -> String {
    let chat_key = match store.get_chat_key(chat_id, message.key_epoch) {
        Ok(Some(key)) => key,
        _ => return "[encrypted - key unavailable]".to_string(),
    };

    let group_key_bytes = match decrypt_key_storage(password, &chat_key.group_key_enc) {
        Ok(bytes) if bytes.len() == 32 => bytes,
        _ => return "[encrypted - decryption failed]".to_string(),
    };

    let mut group_key = [0u8; 32];
    group_key.copy_from_slice(&group_key_bytes);

    let plaintext = match decrypt_message(&group_key, &message.payload_nonce, &message.payload_ciphertext) {
        Ok(data) => data,
        Err(_) => return "[encrypted - decryption failed]".to_string(),
    };

    let payload: MessagePayload = match ciborium::from_reader(plaintext.as_slice()) {
        Ok(p) => p,
        Err(_) => return "[encrypted - invalid payload]".to_string(),
    };

    match payload {
        MessagePayload::Text { body } => body,
        MessagePayload::SystemEvent(_) => "[system event]".to_string(),
    }
}

fn compute_delivery_status(
    store: &ghostmesh_core::store::Store,
    message_id: &[u8; 32],
    chat_id: &[u8; 16],
) -> Result<DeliveryStatus, String> {
    let outbox = store
        .get_outbox_for_chat(chat_id)
        .map_err(|e| e.to_string())?;
    let pending_for_message = outbox
        .iter()
        .filter(|e| e.message_id == *message_id)
        .count() as u32;

    if pending_for_message == 0 {
        return Ok(DeliveryStatus::All);
    }

    let members = store
        .get_chat_members(chat_id)
        .map_err(|e| e.to_string())?;
    let total_targets = members.iter().filter(|m| !m.is_removed).count().saturating_sub(1) as u32;

    if pending_for_message >= total_targets {
        Ok(DeliveryStatus::Queued)
    } else {
        Ok(DeliveryStatus::Partial {
            delivered: total_targets - pending_for_message,
            total: total_targets,
        })
    }
}

fn hex_to_chat_id(hex_str: &str) -> Result<[u8; 16], String> {
    let bytes = hex::decode(hex_str).map_err(|e| format!("invalid chat_id hex: {e}"))?;
    if bytes.len() != 16 {
        return Err(format!(
            "invalid chat_id length: expected 16 bytes, got {}",
            bytes.len()
        ));
    }
    let mut id = [0u8; 16];
    id.copy_from_slice(&bytes);
    Ok(id)
}

fn hex_to_message_id(hex_str: &str) -> Result<[u8; 32], String> {
    let bytes = hex::decode(hex_str).map_err(|e| format!("invalid message_id hex: {e}"))?;
    if bytes.len() != 32 {
        return Err(format!(
            "invalid message_id length: expected 32 bytes, got {}",
            bytes.len()
        ));
    }
    let mut id = [0u8; 32];
    id.copy_from_slice(&bytes);
    Ok(id)
}

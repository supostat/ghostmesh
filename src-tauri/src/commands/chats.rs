use tauri::State;

use ghostmesh_core::crypto::encrypt::{decrypt_key_storage, decrypt_message, encrypt_key_storage};
use ghostmesh_core::store::Store;
use ghostmesh_core::types::{
    Chat, ChatId, ChatInvite, ChatKey, ChatMember, CoreError, GroupKey, MemberRole, Message,
    MessagePayload, PeerId, RekeyPackage, SystemEvent,
};

use crate::state::AppState;
use crate::types::{ChatDetail, ChatInfo, InviteCode, MemberInfo};

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs()
}

#[tauri::command]
pub async fn create_chat(
    state: State<'_, AppState>,
    chat_name: String,
    password: String,
) -> Result<ChatInfo, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;

    let stored_identity = store
        .get_identity()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| CoreError::IdentityNotInitialized.to_string())?;

    let chat_id: [u8; 16] = uuid::Uuid::new_v4().into_bytes();
    let created_at = now_secs();

    let chat = Chat {
        chat_id,
        chat_name: chat_name.clone(),
        owner_peer_id: stored_identity.peer_id,
        created_at,
        my_lamport_counter: 0,
        unread_count: 0,
    };
    store.insert_chat(&chat).map_err(|e| e.to_string())?;

    // Generate group key and store encrypted
    let group_key: GroupKey = rand::random();
    let group_key_enc =
        encrypt_key_storage(&password, &group_key).map_err(|e| e.to_string())?;

    let chat_key = ChatKey {
        chat_id,
        key_epoch: 0,
        group_key_enc,
        created_at,
    };
    store.insert_chat_key(&chat_key).map_err(|e| e.to_string())?;

    // Add self as owner member
    let owner_member = ChatMember {
        chat_id,
        peer_id: stored_identity.peer_id,
        signing_pk: stored_identity.signing_pk,
        exchange_pk: stored_identity.exchange_pk,
        display_name: stored_identity.display_name.clone(),
        role: MemberRole::Owner,
        added_at: created_at,
        added_by: stored_identity.peer_id,
        is_removed: false,
    };
    store
        .insert_chat_member(&owner_member)
        .map_err(|e| e.to_string())?;

    Ok(ChatInfo {
        chat_id: hex::encode(chat_id),
        chat_name,
        owner_peer_id: hex::encode(stored_identity.peer_id),
        created_at,
        member_count: 1,
        online_count: 0,
        last_message_preview: None,
        last_message_at: None,
        unread_count: 0,
        pending_key_exchange: false,
    })
}

#[tauri::command]
pub async fn list_chats(state: State<'_, AppState>) -> Result<Vec<ChatInfo>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let peer_manager = state.peer_manager.lock().map_err(|e| e.to_string())?;
    let session_password = state
        .session_password
        .lock()
        .map_err(|e| e.to_string())?
        .clone();

    let chats = store.list_chats().map_err(|e| e.to_string())?;
    let mut result = Vec::with_capacity(chats.len());

    for chat in &chats {
        let members = store
            .get_chat_members(&chat.chat_id)
            .map_err(|e| e.to_string())?;
        let active_members: Vec<_> = members.iter().filter(|m| !m.is_removed).collect();
        let online_count = active_members
            .iter()
            .filter(|m| peer_manager.is_connected(&m.peer_id))
            .count() as u32;

        let pending_key_exchange = store
            .get_pending_join(&chat.chat_id)
            .map_err(|e| e.to_string())?
            .map(|pj| pj.pending)
            .unwrap_or(false);

        let latest_message = store
            .get_latest_message(&chat.chat_id)
            .unwrap_or(None);

        let (last_message_preview, last_message_at) =
            build_last_message_preview(&store, &chat.chat_id, latest_message.as_ref(), &session_password);

        result.push(ChatInfo {
            chat_id: hex::encode(chat.chat_id),
            chat_name: chat.chat_name.clone(),
            owner_peer_id: hex::encode(chat.owner_peer_id),
            created_at: chat.created_at,
            member_count: active_members.len() as u32,
            online_count,
            last_message_preview,
            last_message_at,
            unread_count: chat.unread_count,
            pending_key_exchange,
        });
    }

    Ok(result)
}

const MAX_PREVIEW_LENGTH: usize = 50;

fn build_last_message_preview(
    store: &ghostmesh_core::store::Store,
    chat_id: &ghostmesh_core::types::ChatId,
    message: Option<&ghostmesh_core::types::Message>,
    session_password: &Option<String>,
) -> (Option<String>, Option<u64>) {
    let message = match message {
        Some(m) => m,
        None => return (None, None),
    };

    let last_message_at = Some(message.created_at);

    let password = match session_password {
        Some(p) => p,
        None => return (None, last_message_at),
    };

    let chat_key = match store.get_chat_key(chat_id, message.key_epoch) {
        Ok(Some(key)) => key,
        _ => return (None, last_message_at),
    };

    let group_key_bytes = match decrypt_key_storage(password, &chat_key.group_key_enc) {
        Ok(bytes) if bytes.len() == 32 => bytes,
        _ => return (None, last_message_at),
    };

    let mut group_key = [0u8; 32];
    group_key.copy_from_slice(&group_key_bytes);

    let plaintext = match decrypt_message(&group_key, &message.payload_nonce, &message.payload_ciphertext) {
        Ok(data) => data,
        Err(_) => return (None, last_message_at),
    };

    let payload: MessagePayload = match ciborium::from_reader(plaintext.as_slice()) {
        Ok(p) => p,
        Err(_) => return (None, last_message_at),
    };

    let preview = match payload {
        MessagePayload::Text { body } => {
            if body.len() > MAX_PREVIEW_LENGTH {
                let truncated: String = body.chars().take(MAX_PREVIEW_LENGTH).collect();
                format!("{truncated}...")
            } else {
                body
            }
        }
        MessagePayload::SystemEvent(_) => "[system event]".to_string(),
    };

    (Some(preview), last_message_at)
}

#[tauri::command]
pub async fn get_chat(
    state: State<'_, AppState>,
    chat_id: String,
) -> Result<ChatDetail, String> {
    let chat_id_bytes = hex_to_chat_id(&chat_id)?;

    let store = state.store.lock().map_err(|e| e.to_string())?;
    let peer_manager = state.peer_manager.lock().map_err(|e| e.to_string())?;

    let chat = store
        .get_chat(&chat_id_bytes)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| CoreError::NotFound("chat not found".to_string()).to_string())?;

    let members = store
        .get_chat_members(&chat_id_bytes)
        .map_err(|e| e.to_string())?;

    let latest_key = store
        .get_latest_chat_key(&chat_id_bytes)
        .map_err(|e| e.to_string())?;
    let key_epoch = latest_key.map(|k| k.key_epoch).unwrap_or(0);

    let member_infos: Vec<MemberInfo> = members
        .iter()
        .filter(|m| !m.is_removed)
        .map(|m| MemberInfo {
            peer_id: hex::encode(m.peer_id),
            display_name: m.display_name.clone(),
            fingerprint: hex::encode(&m.signing_pk[..8]),
            role: m.role.as_str().to_string(),
            is_online: peer_manager.is_connected(&m.peer_id),
            added_at: m.added_at,
        })
        .collect();

    Ok(ChatDetail {
        chat_id: hex::encode(chat.chat_id),
        chat_name: chat.chat_name,
        owner_peer_id: hex::encode(chat.owner_peer_id),
        created_at: chat.created_at,
        members: member_infos,
        key_epoch,
    })
}

#[tauri::command]
pub async fn generate_invite(
    state: State<'_, AppState>,
    chat_id: String,
) -> Result<InviteCode, String> {
    let chat_id_bytes = hex_to_chat_id(&chat_id)?;

    let store = state.store.lock().map_err(|e| e.to_string())?;

    let chat = store
        .get_chat(&chat_id_bytes)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| CoreError::NotFound("chat not found".to_string()).to_string())?;

    let stored_identity = store
        .get_identity()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| CoreError::IdentityNotInitialized.to_string())?;

    let owner_addresses = store
        .get_peer_addresses(&stored_identity.peer_id)
        .map_err(|e| e.to_string())?;

    let invite_token: [u8; 32] = rand::random();

    let invite = ChatInvite {
        chat_id: chat.chat_id,
        chat_name: chat.chat_name,
        owner_peer_id: stored_identity.peer_id,
        owner_signing_pk: stored_identity.signing_pk,
        owner_exchange_pk: stored_identity.exchange_pk,
        owner_addresses,
        invite_token,
        created_at: now_secs(),
    };

    let mut cbor_buffer = Vec::new();
    ciborium::into_writer(&invite, &mut cbor_buffer)
        .map_err(|e| format!("failed to encode invite as CBOR: {e}"))?;

    let encoded = hex::encode(&cbor_buffer);
    let code = format!("ghm://{encoded}");

    Ok(InviteCode { code })
}

#[tauri::command]
pub async fn join_chat(
    state: State<'_, AppState>,
    invite_code: String,
) -> Result<ChatInfo, String> {
    let encoded = invite_code
        .strip_prefix("ghm://")
        .ok_or("invalid invite code: must start with ghm://")?;

    let cbor_bytes =
        hex::decode(encoded).map_err(|e| format!("invalid invite code encoding: {e}"))?;

    let invite: ChatInvite = ciborium::from_reader(cbor_bytes.as_slice())
        .map_err(|e| format!("invalid invite code data: {e}"))?;

    let store = state.store.lock().map_err(|e| e.to_string())?;

    let stored_identity = store
        .get_identity()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| CoreError::IdentityNotInitialized.to_string())?;

    // Check if chat already exists locally
    if store
        .get_chat(&invite.chat_id)
        .map_err(|e| e.to_string())?
        .is_some()
    {
        return Err("already joined this chat".to_string());
    }

    let created_at = now_secs();

    // Store chat locally
    let chat = Chat {
        chat_id: invite.chat_id,
        chat_name: invite.chat_name.clone(),
        owner_peer_id: invite.owner_peer_id,
        created_at: invite.created_at,
        my_lamport_counter: 0,
        unread_count: 0,
    };
    store.insert_chat(&chat).map_err(|e| e.to_string())?;

    // Add owner as known member
    let owner_member = ChatMember {
        chat_id: invite.chat_id,
        peer_id: invite.owner_peer_id,
        signing_pk: invite.owner_signing_pk,
        exchange_pk: invite.owner_exchange_pk,
        display_name: String::new(), // Will be updated during sync
        role: MemberRole::Owner,
        added_at: invite.created_at,
        added_by: invite.owner_peer_id,
        is_removed: false,
    };
    store
        .insert_chat_member(&owner_member)
        .map_err(|e| e.to_string())?;

    // Add self as member
    let self_member = ChatMember {
        chat_id: invite.chat_id,
        peer_id: stored_identity.peer_id,
        signing_pk: stored_identity.signing_pk,
        exchange_pk: stored_identity.exchange_pk,
        display_name: stored_identity.display_name.clone(),
        role: MemberRole::Member,
        added_at: created_at,
        added_by: stored_identity.peer_id,
        is_removed: false,
    };
    store
        .insert_chat_member(&self_member)
        .map_err(|e| e.to_string())?;

    // Store owner's addresses for future connection
    for address in &invite.owner_addresses {
        store
            .upsert_peer_address(address)
            .map_err(|e| e.to_string())?;
    }

    // Mark as pending key exchange — group key will arrive via JoinResponse
    store
        .insert_pending_join(&invite.chat_id, &invite.invite_token)
        .map_err(|e| e.to_string())?;

    Ok(ChatInfo {
        chat_id: hex::encode(invite.chat_id),
        chat_name: invite.chat_name,
        owner_peer_id: hex::encode(invite.owner_peer_id),
        created_at: invite.created_at,
        member_count: 2,
        online_count: 0,
        last_message_preview: None,
        last_message_at: None,
        unread_count: 0,
        pending_key_exchange: true,
    })
}

#[tauri::command]
pub async fn leave_chat(
    state: State<'_, AppState>,
    chat_id: String,
) -> Result<(), String> {
    let chat_id_bytes = hex_to_chat_id(&chat_id)?;

    let store = state.store.lock().map_err(|e| e.to_string())?;

    let stored_identity = store
        .get_identity()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| CoreError::IdentityNotInitialized.to_string())?;

    store
        .remove_chat_member(&chat_id_bytes, &stored_identity.peer_id)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn remove_member(
    state: State<'_, AppState>,
    chat_id: String,
    peer_id: String,
    password: String,
) -> Result<(), String> {
    let chat_id_bytes = hex_to_chat_id(&chat_id)?;
    let target_peer_id = hex_to_peer_id(&peer_id)?;

    let store = state.store.lock().map_err(|e| e.to_string())?;
    let mut lamport = state.lamport.lock().map_err(|e| e.to_string())?;

    let stored_identity = store
        .get_identity()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| CoreError::IdentityNotInitialized.to_string())?;

    let chat = store
        .get_chat(&chat_id_bytes)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| CoreError::NotFound("chat not found".to_string()).to_string())?;

    if chat.owner_peer_id != stored_identity.peer_id {
        return Err("only the chat owner can remove members".to_string());
    }

    if target_peer_id == stored_identity.peer_id {
        return Err("owner cannot remove themselves".to_string());
    }

    // Decrypt owner's exchange secret key
    let exchange_sk_bytes =
        decrypt_key_storage(&password, &stored_identity.exchange_sk_enc)
            .map_err(|e| e.to_string())?;
    if exchange_sk_bytes.len() != 32 {
        return Err("invalid exchange key length after decryption".to_string());
    }
    let mut exchange_sk = [0u8; 32];
    exchange_sk.copy_from_slice(&exchange_sk_bytes);

    // Decrypt owner's signing secret key
    let signing_sk_bytes =
        decrypt_key_storage(&password, &stored_identity.signing_sk_enc)
            .map_err(|e| e.to_string())?;
    if signing_sk_bytes.len() != 64 {
        return Err("invalid signing key length after decryption".to_string());
    }
    let mut signing_sk = [0u8; 64];
    signing_sk.copy_from_slice(&signing_sk_bytes);

    // Remove member from store
    store
        .remove_chat_member(&chat_id_bytes, &target_peer_id)
        .map_err(|e| e.to_string())?;

    // Generate new group key
    let new_group_key: [u8; 32] = rand::random();

    // Get new epoch
    let new_key_epoch = store
        .get_latest_chat_key(&chat_id_bytes)
        .map_err(|e| e.to_string())?
        .map(|k| k.key_epoch + 1)
        .unwrap_or(0);

    // Store new key locally encrypted with owner's password
    let new_group_key_enc =
        encrypt_key_storage(&password, &new_group_key).map_err(|e| e.to_string())?;

    let now = now_secs();
    store
        .insert_chat_key(&ChatKey {
            chat_id: chat_id_bytes,
            key_epoch: new_key_epoch,
            group_key_enc: new_group_key_enc,
            created_at: now,
        })
        .map_err(|e| e.to_string())?;

    // Build rekey packages for remaining active members
    let rekey_packages =
        build_rekey_packages(&store, &chat_id_bytes, &new_group_key, &exchange_sk)?;

    // Build and store system event message
    let lamport_ts = lamport.on_send();
    let message = build_member_removed_message(
        &chat_id_bytes,
        &target_peer_id,
        new_key_epoch,
        rekey_packages,
        &new_group_key,
        &signing_sk,
        &stored_identity.peer_id,
        lamport_ts,
        now,
    )?;

    let message_id = message.message_id;
    store
        .insert_message(&message)
        .map_err(|e| e.to_string())?;

    // Add to outbox for remaining active members
    let members = store
        .get_chat_members(&chat_id_bytes)
        .map_err(|e| e.to_string())?;

    for member in &members {
        if member.is_removed || member.peer_id == stored_identity.peer_id {
            continue;
        }
        let entry = ghostmesh_core::types::OutboxEntry {
            message_id,
            target_peer_id: member.peer_id,
            chat_id: chat_id_bytes,
            created_at: now,
        };
        store
            .insert_outbox_entry(&entry)
            .map_err(|e| e.to_string())?;
    }

    Ok(())
}

#[tauri::command]
pub async fn set_member_role(
    state: State<'_, AppState>,
    chat_id: String,
    peer_id: String,
    role: String,
) -> Result<(), String> {
    let chat_id_bytes = hex_to_chat_id(&chat_id)?;
    let peer_id_bytes = hex_to_peer_id(&peer_id)?;

    let new_role = MemberRole::from_str(&role)
        .ok_or_else(|| format!("invalid role: expected 'admin' or 'member', got '{role}'"))?;
    if new_role == MemberRole::Owner {
        return Err("cannot assign owner role via set_member_role".to_string());
    }

    let store = state.store.lock().map_err(|e| e.to_string())?;

    let stored_identity = store
        .get_identity()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| CoreError::IdentityNotInitialized.to_string())?;

    let chat = store
        .get_chat(&chat_id_bytes)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| CoreError::NotFound("chat not found".to_string()).to_string())?;

    if chat.owner_peer_id != stored_identity.peer_id {
        return Err("only the chat owner can change member roles".to_string());
    }

    store
        .update_member_role(&chat_id_bytes, &peer_id_bytes, new_role)
        .map_err(|e| e.to_string())?;

    Ok(())
}

fn build_rekey_packages(
    store: &Store,
    chat_id: &ChatId,
    new_group_key: &[u8; 32],
    exchange_sk: &[u8; 32],
) -> Result<Vec<RekeyPackage>, String> {
    let members = store
        .get_chat_members(chat_id)
        .map_err(|e| e.to_string())?;

    let self_identity = store
        .get_identity()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| CoreError::IdentityNotInitialized.to_string())?;

    let mut packages = Vec::new();
    for member in &members {
        if member.is_removed || member.peer_id == self_identity.peer_id {
            continue;
        }
        let shared_secret =
            ghostmesh_core::crypto::exchange::derive_shared_secret(exchange_sk, &member.exchange_pk)
                .map_err(|e| e.to_string())?;
        let encrypted_key =
            ghostmesh_core::crypto::encrypt::wrap_key(new_group_key, &shared_secret)
                .map_err(|e| e.to_string())?;
        packages.push(RekeyPackage {
            target_peer_id: member.peer_id,
            encrypted_key,
        });
    }
    Ok(packages)
}

fn build_member_removed_message(
    chat_id: &ChatId,
    removed_peer_id: &PeerId,
    new_key_epoch: u64,
    rekey_packages: Vec<RekeyPackage>,
    new_group_key: &[u8; 32],
    signing_sk: &[u8; 64],
    author_peer_id: &PeerId,
    lamport_ts: u64,
    now: u64,
) -> Result<Message, String> {
    let payload = MessagePayload::SystemEvent(SystemEvent::MemberRemoved {
        peer_id: *removed_peer_id,
        new_key_epoch,
        rekey_packages,
    });

    let mut payload_cbor = Vec::new();
    ciborium::into_writer(&payload, &mut payload_cbor)
        .map_err(|e| format!("failed to encode system event payload: {e}"))?;

    let (ciphertext, nonce) =
        ghostmesh_core::crypto::encrypt::encrypt_message(new_group_key, &payload_cbor)
            .map_err(|e| e.to_string())?;

    let message_id: [u8; 32] = rand::random();

    let mut signable = Vec::new();
    signable.extend_from_slice(&message_id);
    signable.extend_from_slice(chat_id);
    signable.extend_from_slice(author_peer_id);
    signable.extend_from_slice(&lamport_ts.to_le_bytes());
    signable.extend_from_slice(&ciphertext);
    signable.extend_from_slice(&nonce);

    let signature =
        ghostmesh_core::crypto::sign::sign(signing_sk, &signable).map_err(|e| e.to_string())?;

    Ok(Message {
        message_id,
        chat_id: *chat_id,
        author_peer_id: *author_peer_id,
        lamport_ts,
        created_at: now,
        key_epoch: new_key_epoch,
        parent_ids: Vec::new(),
        signature,
        payload_ciphertext: ciphertext,
        payload_nonce: nonce,
        received_at: now,
    })
}

fn hex_to_peer_id(hex_str: &str) -> Result<[u8; 16], String> {
    let bytes = hex::decode(hex_str).map_err(|e| format!("invalid peer_id hex: {e}"))?;
    if bytes.len() != 16 {
        return Err(format!(
            "invalid peer_id length: expected 16 bytes, got {}",
            bytes.len()
        ));
    }
    let mut peer_id = [0u8; 16];
    peer_id.copy_from_slice(&bytes);
    Ok(peer_id)
}

fn hex_to_chat_id(hex_str: &str) -> Result<[u8; 16], String> {
    let bytes = hex::decode(hex_str).map_err(|e| format!("invalid chat_id hex: {e}"))?;
    if bytes.len() != 16 {
        return Err(format!(
            "invalid chat_id length: expected 16 bytes, got {}",
            bytes.len()
        ));
    }
    let mut chat_id = [0u8; 16];
    chat_id.copy_from_slice(&bytes);
    Ok(chat_id)
}

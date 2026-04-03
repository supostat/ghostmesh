use tauri::State;

use ghostmesh_core::crypto::encrypt::encrypt_key_storage;
use ghostmesh_core::types::{
    Chat, ChatInvite, ChatKey, ChatMember, CoreError, GroupKey, MemberRole,
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

        result.push(ChatInfo {
            chat_id: hex::encode(chat.chat_id),
            chat_name: chat.chat_name.clone(),
            owner_peer_id: hex::encode(chat.owner_peer_id),
            created_at: chat.created_at,
            member_count: active_members.len() as u32,
            online_count,
            last_message_preview: None,
            last_message_at: None,
            unread_count: 0,
            pending_key_exchange,
        });
    }

    Ok(result)
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

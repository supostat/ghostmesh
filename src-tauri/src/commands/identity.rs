use tauri::State;

use ghostmesh_core::crypto::encrypt::{decrypt_key_storage, encrypt_key_storage};
use ghostmesh_core::crypto::identity::{generate_identity, derive_peer_id};
use ghostmesh_core::types::CoreError;

use crate::state::AppState;
use crate::types::IdentityInfo;

fn fingerprint_from_signing_pk(signing_pk: &[u8; 32]) -> String {
    let full_hex = hex::encode(signing_pk);
    full_hex[..16].to_string()
}

fn stored_identity_to_info(
    stored: &ghostmesh_core::store::StoredIdentity,
) -> IdentityInfo {
    IdentityInfo {
        peer_id: hex::encode(stored.peer_id),
        display_name: stored.display_name.clone(),
        fingerprint: fingerprint_from_signing_pk(&stored.signing_pk),
        created_at: stored.created_at,
    }
}

#[tauri::command]
pub async fn create_identity(
    state: State<'_, AppState>,
    display_name: String,
    password: String,
) -> Result<IdentityInfo, String> {
    let identity = generate_identity(display_name);

    let signing_sk_enc = encrypt_key_storage(&password, &identity.signing_keypair.secret)
        .map_err(|e| e.to_string())?;
    let exchange_sk_enc = encrypt_key_storage(&password, &identity.exchange_keypair.secret)
        .map_err(|e| e.to_string())?;

    let store = state.store.lock().map_err(|e| e.to_string())?;
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
        .map_err(|e| e.to_string())?;
    drop(store);

    let mut session_pw = state.session_password.lock().map_err(|e| e.to_string())?;
    *session_pw = Some(password);

    Ok(IdentityInfo {
        peer_id: hex::encode(identity.peer_id),
        display_name: identity.display_name,
        fingerprint: fingerprint_from_signing_pk(&identity.signing_keypair.public),
        created_at: identity.created_at,
    })
}

#[tauri::command]
pub async fn get_identity(state: State<'_, AppState>) -> Result<IdentityInfo, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let stored = store
        .get_identity()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| CoreError::IdentityNotInitialized.to_string())?;

    Ok(stored_identity_to_info(&stored))
}

#[tauri::command]
pub async fn validate_password(
    state: State<'_, AppState>,
    password: String,
) -> Result<bool, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let stored = store
        .get_identity()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| CoreError::IdentityNotInitialized.to_string())?;

    match decrypt_key_storage(&password, &stored.signing_sk_enc) {
        Ok(bytes) if bytes.len() == 64 => {
            let mut session_pw = state.session_password.lock().map_err(|e| e.to_string())?;
            *session_pw = Some(password);
            Ok(true)
        }
        _ => Ok(false),
    }
}

#[tauri::command]
pub async fn export_identity(
    state: State<'_, AppState>,
    password: String,
) -> Result<Vec<u8>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let stored = store
        .get_identity()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| CoreError::IdentityNotInitialized.to_string())?;

    let export_data = serde_json::to_vec(&serde_json::json!({
        "peer_id": hex::encode(stored.peer_id),
        "signing_sk_enc": stored.signing_sk_enc,
        "signing_pk": stored.signing_pk.to_vec(),
        "exchange_sk_enc": stored.exchange_sk_enc,
        "exchange_pk": stored.exchange_pk.to_vec(),
        "display_name": stored.display_name,
        "created_at": stored.created_at,
    }))
    .map_err(|e| format!("failed to serialize identity for export: {e}"))?;

    encrypt_key_storage(&password, &export_data).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn import_identity(
    state: State<'_, AppState>,
    encrypted_export: Vec<u8>,
    password: String,
) -> Result<IdentityInfo, String> {
    let decrypted = decrypt_key_storage(&password, &encrypted_export).map_err(|e| e.to_string())?;

    let parsed: serde_json::Value =
        serde_json::from_slice(&decrypted).map_err(|e| format!("invalid identity export: {e}"))?;

    let peer_id_hex = parsed["peer_id"]
        .as_str()
        .ok_or("missing peer_id in export")?;
    let peer_id_bytes = hex::decode(peer_id_hex).map_err(|e| format!("invalid peer_id hex: {e}"))?;
    let mut peer_id = [0u8; 16];
    if peer_id_bytes.len() != 16 {
        return Err("invalid peer_id length".to_string());
    }
    peer_id.copy_from_slice(&peer_id_bytes);

    let signing_sk_enc: Vec<u8> = serde_json::from_value(parsed["signing_sk_enc"].clone())
        .map_err(|e| format!("invalid signing_sk_enc: {e}"))?;

    let signing_pk_vec: Vec<u8> = serde_json::from_value(parsed["signing_pk"].clone())
        .map_err(|e| format!("invalid signing_pk: {e}"))?;
    let mut signing_pk = [0u8; 32];
    if signing_pk_vec.len() != 32 {
        return Err("invalid signing_pk length".to_string());
    }
    signing_pk.copy_from_slice(&signing_pk_vec);

    let exchange_sk_enc: Vec<u8> = serde_json::from_value(parsed["exchange_sk_enc"].clone())
        .map_err(|e| format!("invalid exchange_sk_enc: {e}"))?;

    let exchange_pk_vec: Vec<u8> = serde_json::from_value(parsed["exchange_pk"].clone())
        .map_err(|e| format!("invalid exchange_pk: {e}"))?;
    let mut exchange_pk = [0u8; 32];
    if exchange_pk_vec.len() != 32 {
        return Err("invalid exchange_pk length".to_string());
    }
    exchange_pk.copy_from_slice(&exchange_pk_vec);

    let display_name = parsed["display_name"]
        .as_str()
        .ok_or("missing display_name in export")?
        .to_string();

    let created_at = parsed["created_at"]
        .as_u64()
        .ok_or("missing created_at in export")?;

    // Verify peer_id matches signing_pk
    let derived = derive_peer_id(&signing_pk);
    if derived != peer_id {
        return Err("peer_id does not match signing public key".to_string());
    }

    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .save_identity(
            &peer_id,
            &signing_sk_enc,
            &signing_pk,
            &exchange_sk_enc,
            &exchange_pk,
            &display_name,
            created_at,
        )
        .map_err(|e| e.to_string())?;

    Ok(IdentityInfo {
        peer_id: hex::encode(peer_id),
        display_name,
        fingerprint: fingerprint_from_signing_pk(&signing_pk),
        created_at,
    })
}

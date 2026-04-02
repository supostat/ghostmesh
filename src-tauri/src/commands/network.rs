use tauri::State;

use ghostmesh_core::types::PeerAddress;

use crate::state::AppState;
use crate::types::{ConnectionInfo, OutboxInfo, PeerInfo, SyncLogInfo};

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_secs()
}

#[tauri::command]
pub async fn get_peers(state: State<'_, AppState>) -> Result<Vec<PeerInfo>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    let peer_manager = state.peer_manager.lock().map_err(|e| e.to_string())?;

    let all_addresses = store
        .get_all_peer_addresses()
        .map_err(|e| e.to_string())?;

    // Group addresses by peer_id
    let mut peers: std::collections::HashMap<[u8; 16], Vec<PeerAddress>> =
        std::collections::HashMap::new();
    for addr in all_addresses {
        peers.entry(addr.peer_id).or_default().push(addr);
    }

    let mut result = Vec::with_capacity(peers.len());
    for (peer_id, addresses) in &peers {
        let last_seen = addresses.iter().map(|a| a.last_seen).max();
        let address_strings: Vec<String> = addresses
            .iter()
            .map(|a| format!("{}:{}", a.address_type, a.address))
            .collect();

        result.push(PeerInfo {
            peer_id: hex::encode(peer_id),
            display_name: String::new(), // No display_name in peer_addresses table
            addresses: address_strings,
            last_seen,
            is_connected: peer_manager.is_connected(peer_id),
        });
    }

    Ok(result)
}

#[tauri::command]
pub async fn get_connections(state: State<'_, AppState>) -> Result<Vec<ConnectionInfo>, String> {
    let peer_manager = state.peer_manager.lock().map_err(|e| e.to_string())?;

    let connected_peers = peer_manager.connected_peers();
    let mut result = Vec::with_capacity(connected_peers.len());

    for peer_id in &connected_peers {
        let connection_info = peer_manager.get_connection_info(peer_id);
        if let Some(info) = connection_info {
            result.push(ConnectionInfo {
                peer_id: hex::encode(peer_id),
                display_name: String::new(),
                address: info.address.clone(),
                connected_at: info.connected_at,
                messages_synced: 0, // Tracked by sync engine in future
            });
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn get_outbox(state: State<'_, AppState>) -> Result<Vec<OutboxInfo>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;

    // Get all outbox entries by querying for each known peer's entries
    // Since there's no get_all_outbox, we use outbox_for_chat for all chats
    let chats = store.list_chats().map_err(|e| e.to_string())?;
    let mut result = Vec::new();

    for chat in &chats {
        let entries = store
            .get_outbox_for_chat(&chat.chat_id)
            .map_err(|e| e.to_string())?;
        for entry in &entries {
            result.push(OutboxInfo {
                message_id: hex::encode(entry.message_id),
                target_peer_id: hex::encode(entry.target_peer_id),
                chat_id: hex::encode(entry.chat_id),
                created_at: entry.created_at,
            });
        }
    }

    Ok(result)
}

#[tauri::command]
pub async fn add_manual_peer(
    state: State<'_, AppState>,
    peer_id: String,
    address: String,
) -> Result<(), String> {
    let peer_id_bytes = hex_to_peer_id(&peer_id)?;

    let store = state.store.lock().map_err(|e| e.to_string())?;

    let peer_address = PeerAddress {
        peer_id: peer_id_bytes,
        address_type: "tcp".to_string(),
        address,
        last_seen: now_secs(),
        last_successful: None,
        fail_count: 0,
    };

    store
        .upsert_peer_address(&peer_address)
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn get_sync_log(
    state: State<'_, AppState>,
    limit: Option<u32>,
) -> Result<Vec<SyncLogInfo>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;

    let entries = store
        .get_sync_log(limit.unwrap_or(100))
        .map_err(|e| e.to_string())?;

    let result: Vec<SyncLogInfo> = entries
        .iter()
        .map(|e| SyncLogInfo {
            id: e.id,
            timestamp: e.timestamp,
            peer_id: e.peer_id.map(|p| hex::encode(p)),
            event_type: e.event_type.clone(),
            detail: e.detail.clone(),
        })
        .collect();

    Ok(result)
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

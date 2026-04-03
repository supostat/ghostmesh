use tauri::{AppHandle, Manager};

use crate::state::AppState;

const JOIN_POLL_INTERVAL_SECS: u64 = 5;
const MAX_JOIN_RETRIES: u32 = 50;

/// Background loop that checks for pending joins and processes them
/// when the chat owner comes online.
///
/// The actual JoinRequest/JoinResponse exchange over the wire
/// requires the full transport layer (SecureConnection). This
/// orchestrator detects when the owner is online and logs the
/// pending request. The core join logic lives in
/// SyncEngine::handle_join_request / handle_join_response —
/// they are tested and ready to wire into the transport.
pub async fn run_join_orchestrator(app: AppHandle) {
    loop {
        if let Err(error) = check_pending_joins(&app) {
            tracing::warn!("join orchestrator error: {error}");
        }
        tokio::time::sleep(std::time::Duration::from_secs(JOIN_POLL_INTERVAL_SECS)).await;
    }
}

fn check_pending_joins(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();

    let store = state.store.lock().map_err(|e| e.to_string())?;
    let peer_manager = state.peer_manager.lock().map_err(|e| e.to_string())?;
    let session_password = state.session_password.lock().map_err(|e| e.to_string())?;

    if session_password.is_none() {
        return Ok(()); // Identity not unlocked yet
    }

    if store.get_identity().map_err(|e| e.to_string())?.is_none() {
        return Ok(()); // No identity yet
    }

    let chats = store.list_chats().map_err(|e| e.to_string())?;

    for chat in &chats {
        let pending = store
            .get_pending_join(&chat.chat_id)
            .map_err(|e| e.to_string())?;

        let pending_join = match pending {
            Some(pj) if pj.pending && pj.retry_count < MAX_JOIN_RETRIES => pj,
            _ => continue,
        };

        if !peer_manager.is_connected(&chat.owner_peer_id) {
            continue;
        }

        // Owner is online — increment retry counter.
        // The actual wire JoinRequest will be sent once transport
        // message routing is implemented.
        store
            .increment_pending_join_retry(&chat.chat_id)
            .map_err(|e| e.to_string())?;

        tracing::info!(
            "join orchestrator: owner of chat {} is online, join request pending (retry {})",
            hex::encode(chat.chat_id),
            pending_join.retry_count + 1,
        );
    }

    Ok(())
}

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

        // Owner is online — send JoinRequest via network layer
        let identity = match store.get_identity().map_err(|e| e.to_string())? {
            Some(id) => id,
            None => continue,
        };

        let network_tx_guard = state.network_tx.lock().map_err(|e| e.to_string())?;
        if let Some(network_tx) = network_tx_guard.as_ref() {
            use ghostmesh_core::net::NetworkCommand;
            use ghostmesh_core::types::{PeerIdentityPacket, WireMessage};

            let join_request = WireMessage::JoinRequest {
                chat_id: chat.chat_id,
                invite_token: pending_join.invite_token,
                identity: PeerIdentityPacket {
                    peer_id: identity.peer_id,
                    signing_pk: identity.signing_pk,
                    exchange_pk: identity.exchange_pk,
                    display_name: identity.display_name.clone(),
                },
            };

            let tx = network_tx.clone();
            drop(network_tx_guard);

            if let Err(error) = tx.try_send(NetworkCommand::SendMessage {
                peer_id: chat.owner_peer_id,
                message: join_request,
            }) {
                tracing::warn!(
                    "join orchestrator: failed to send JoinRequest for chat {}: {error}",
                    hex::encode(chat.chat_id),
                );
            }
        } else {
            drop(network_tx_guard);
        }

        store
            .increment_pending_join_retry(&chat.chat_id)
            .map_err(|e| e.to_string())?;

        tracing::info!(
            "join orchestrator: sent JoinRequest for chat {} (retry {})",
            hex::encode(chat.chat_id),
            pending_join.retry_count + 1,
        );
    }

    Ok(())
}

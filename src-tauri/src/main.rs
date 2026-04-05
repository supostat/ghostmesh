#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod events;
mod join_orchestrator;
mod state;
mod tauri_event_sink;
mod types;

use std::sync::{Arc, Mutex};
use std::time::Duration;

use tauri::{Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

use ghostmesh_core::crypto::noise::generate_noise_keypair;
use ghostmesh_core::net::{NetworkService, PeerManager};
use ghostmesh_core::store::Store;
use ghostmesh_core::sync::LamportClock;
use ghostmesh_core::types::Settings;

use crate::state::AppState;
use crate::tauri_event_sink::TauriEventSink;

fn main() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("failed to resolve app data dir: {e}"))?;

            std::fs::create_dir_all(&app_data_dir)
                .map_err(|e| format!("failed to create app data dir: {e}"))?;

            let db_path = app_data_dir.join("ghostmesh.db");
            let db_path_str = db_path
                .to_str()
                .ok_or("app data dir path is not valid UTF-8")?;

            let store =
                Store::open(db_path_str).map_err(|e| format!("failed to open database: {e}"))?;

            let app_state = AppState {
                store: Mutex::new(store),
                lamport: Mutex::new(LamportClock::new()),
                peer_manager: Mutex::new(PeerManager::new()),
                settings: Mutex::new(Settings::default()),
                session_password: Mutex::new(None),
                network_tx: Mutex::new(None),
            };

            app.manage(app_state);

            let net_handle = app.handle().clone();
            let net_db_path = db_path_str.to_string();
            tauri::async_runtime::spawn(async move {
                spawn_network_service(net_handle, net_db_path).await;
            });

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                join_orchestrator::run_join_orchestrator(handle).await;
            });

            let cleanup_db_path = db_path_str.to_string();
            let cleanup_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                run_periodic_cleanup(cleanup_db_path, cleanup_handle).await;
            });

            let update_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                check_for_update_on_startup(update_handle).await;
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Identity (5)
            commands::identity::create_identity,
            commands::identity::get_identity,
            commands::identity::validate_password,
            commands::identity::export_identity,
            commands::identity::import_identity,
            // Chats (6)
            commands::chats::create_chat,
            commands::chats::list_chats,
            commands::chats::get_chat,
            commands::chats::generate_invite,
            commands::chats::join_chat,
            commands::chats::leave_chat,
            // Messages (3)
            commands::messages::send_message,
            commands::messages::get_messages,
            commands::messages::get_message_detail,
            // Network (5)
            commands::network::get_peers,
            commands::network::get_connections,
            commands::network::get_outbox,
            commands::network::add_manual_peer,
            commands::network::get_sync_log,
            // Settings (2)
            commands::settings::get_settings,
            commands::settings::update_settings,
            // Updates (2)
            commands::updates::check_for_update,
            commands::updates::install_update,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn spawn_network_service(app: tauri::AppHandle, db_path: String) {
    let state = app.state::<AppState>();

    let store_for_net = match Store::open(&db_path) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("network service: failed to open store: {e}");
            return;
        }
    };

    let identity = match store_for_net.get_identity() {
        Ok(Some(id)) => id,
        Ok(None) => {
            tracing::info!("network service: no identity yet, skipping startup");
            return;
        }
        Err(e) => {
            tracing::warn!("network service: failed to load identity: {e}");
            return;
        }
    };

    let noise_keypair = match generate_noise_keypair() {
        Ok(kp) => kp,
        Err(e) => {
            tracing::error!("network service: failed to generate noise keypair: {e}");
            return;
        }
    };

    let listen_port = state
        .settings
        .lock()
        .map(|s| s.listen_port)
        .unwrap_or(9473);

    let bind_address = format!("0.0.0.0:{listen_port}");

    let peer_manager = Arc::new(Mutex::new(PeerManager::new()));
    let store_arc = Arc::new(Mutex::new(store_for_net));
    let lamport_arc = Arc::new(Mutex::new(LamportClock::new()));
    let session_password_arc = Arc::new(Mutex::new(
        state.session_password.lock().ok().and_then(|p| p.clone()),
    ));

    let (service, command_tx) = NetworkService::new(
        bind_address,
        identity.peer_id,
        noise_keypair,
        identity.signing_pk,
        peer_manager,
        store_arc,
        lamport_arc,
        session_password_arc,
    );

    {
        if let Ok(mut tx_guard) = state.network_tx.lock() {
            *tx_guard = Some(command_tx);
        }
    }

    let event_sink = Arc::new(TauriEventSink::new(app));
    service.run(event_sink).await;
}

const CLEANUP_INTERVAL: Duration = Duration::from_secs(300);
const STALE_PEER_ADDRESS_MAX_AGE_SECS: u64 = 86400 * 30;
const STALE_PEER_ADDRESS_MIN_FAILURES: u32 = 10;

async fn run_periodic_cleanup(db_path: String, app: tauri::AppHandle) {
    let store = match Store::open(&db_path) {
        Ok(s) => s,
        Err(error) => {
            tracing::warn!("cleanup task: failed to open store: {error}");
            return;
        }
    };

    let mut interval = tokio::time::interval(CLEANUP_INTERVAL);
    loop {
        interval.tick().await;
        match store.cleanup_stale_peer_addresses(
            STALE_PEER_ADDRESS_MAX_AGE_SECS,
            STALE_PEER_ADDRESS_MIN_FAILURES,
        ) {
            Ok(removed) => {
                if removed > 0 {
                    tracing::info!("cleanup: removed {removed} stale peer addresses");
                }
            }
            Err(error) => {
                tracing::debug!("cleanup: failed to clean peer addresses: {error}");
            }
        }

        let ttl_days = app
            .state::<AppState>()
            .settings
            .lock()
            .ok()
            .and_then(|s| s.message_ttl_days);

        if let Some(ttl) = ttl_days {
            match store.delete_old_messages(ttl) {
                Ok(deleted) => {
                    if deleted > 0 {
                        tracing::info!("cleanup: deleted {deleted} expired messages (ttl={ttl} days)");
                    }
                }
                Err(error) => {
                    tracing::debug!("cleanup: failed to delete old messages: {error}");
                }
            }
        }
    }
}

async fn check_for_update_on_startup(app: tauri::AppHandle) {
    let auto_update_enabled = app
        .state::<AppState>()
        .settings
        .lock()
        .map(|s| s.auto_update_enabled)
        .unwrap_or(false);

    if !auto_update_enabled {
        return;
    }

    let updater = match app.updater() {
        Ok(u) => u,
        Err(e) => {
            tracing::warn!("updater not available: {e}");
            return;
        }
    };

    match updater.check().await {
        Ok(Some(update)) => {
            tracing::info!("update available: v{}", update.version);
            let _ = app.emit("update:available", &commands::updates::UpdateInfo {
                version: update.version.clone(),
                body: update.body.clone(),
                date: update.date.map(|d| d.to_string()),
            });
        }
        Ok(None) => {
            tracing::debug!("no update available");
        }
        Err(e) => {
            tracing::warn!("failed to check for updates: {e}");
        }
    }
}

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod events;
mod join_orchestrator;
mod state;
mod types;

use std::sync::Mutex;

use tauri::Manager;

use ghostmesh_core::net::PeerManager;
use ghostmesh_core::store::Store;
use ghostmesh_core::sync::LamportClock;
use ghostmesh_core::types::Settings;

use crate::state::AppState;

fn main() {
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
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
            };

            app.manage(app_state);

            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                join_orchestrator::run_join_orchestrator(handle).await;
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

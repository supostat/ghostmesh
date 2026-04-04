use std::sync::Mutex;

use ghostmesh_core::net::{NetworkCommand, PeerManager};
use ghostmesh_core::store::Store;
use ghostmesh_core::sync::LamportClock;
use ghostmesh_core::types::Settings;

pub struct AppState {
    pub store: Mutex<Store>,
    pub lamport: Mutex<LamportClock>,
    pub peer_manager: Mutex<PeerManager>,
    pub settings: Mutex<Settings>,
    pub session_password: Mutex<Option<String>>,
    pub network_tx: Mutex<Option<tokio::sync::mpsc::Sender<NetworkCommand>>>,
}

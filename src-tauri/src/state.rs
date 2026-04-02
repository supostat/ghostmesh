use std::sync::Mutex;

use ghostmesh_core::store::Store;
use ghostmesh_core::sync::LamportClock;
use ghostmesh_core::net::PeerManager;
use ghostmesh_core::types::Settings;

pub struct AppState {
    pub store: Mutex<Store>,
    pub lamport: Mutex<LamportClock>,
    pub peer_manager: Mutex<PeerManager>,
    pub settings: Mutex<Settings>,
}

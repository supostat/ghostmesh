use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::PeerId;

#[derive(Debug, Clone)]
pub struct PeerConnectionInfo {
    pub address: String,
    pub connected_at: u64,
}

pub struct PeerManager {
    connections: HashMap<PeerId, PeerConnectionInfo>,
}

impl PeerManager {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    pub fn add_connection(&mut self, peer_id: PeerId, address: String) -> bool {
        if self.connections.contains_key(&peer_id) {
            return false;
        }
        let connected_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        self.connections.insert(
            peer_id,
            PeerConnectionInfo {
                address,
                connected_at,
            },
        );
        true
    }

    pub fn remove_connection(&mut self, peer_id: &PeerId) -> Option<String> {
        self.connections
            .remove(peer_id)
            .map(|info| info.address)
    }

    pub fn is_connected(&self, peer_id: &PeerId) -> bool {
        self.connections.contains_key(peer_id)
    }

    pub fn connected_peers(&self) -> Vec<PeerId> {
        self.connections.keys().copied().collect()
    }

    pub fn connected_count(&self) -> usize {
        self.connections.len()
    }

    pub fn get_address(&self, peer_id: &PeerId) -> Option<&str> {
        self.connections
            .get(peer_id)
            .map(|info| info.address.as_str())
    }

    pub fn get_connection_info(&self, peer_id: &PeerId) -> Option<&PeerConnectionInfo> {
        self.connections.get(peer_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PEER_A: PeerId = [0xAA; 16];
    const PEER_B: PeerId = [0xBB; 16];
    const PEER_C: PeerId = [0xCC; 16];

    #[test]
    fn new_manager_has_no_connections() {
        let manager = PeerManager::new();
        assert_eq!(manager.connected_count(), 0);
        assert!(manager.connected_peers().is_empty());
    }

    #[test]
    fn add_connection_succeeds() {
        let mut manager = PeerManager::new();
        let added = manager.add_connection(PEER_A, "127.0.0.1:9473".to_string());
        assert!(added);
        assert_eq!(manager.connected_count(), 1);
        assert!(manager.is_connected(&PEER_A));
    }

    #[test]
    fn add_duplicate_connection_rejected() {
        let mut manager = PeerManager::new();
        manager.add_connection(PEER_A, "127.0.0.1:9473".to_string());
        let duplicate = manager.add_connection(PEER_A, "192.168.1.1:9473".to_string());
        assert!(!duplicate);
        assert_eq!(manager.connected_count(), 1);
        assert_eq!(
            manager.get_address(&PEER_A).unwrap(),
            "127.0.0.1:9473"
        );
    }

    #[test]
    fn remove_existing_connection() {
        let mut manager = PeerManager::new();
        manager.add_connection(PEER_A, "127.0.0.1:9473".to_string());
        let removed = manager.remove_connection(&PEER_A);
        assert_eq!(removed.unwrap(), "127.0.0.1:9473");
        assert!(!manager.is_connected(&PEER_A));
        assert_eq!(manager.connected_count(), 0);
    }

    #[test]
    fn remove_nonexistent_connection_returns_none() {
        let mut manager = PeerManager::new();
        let removed = manager.remove_connection(&PEER_A);
        assert!(removed.is_none());
    }

    #[test]
    fn is_connected_reflects_state() {
        let mut manager = PeerManager::new();
        assert!(!manager.is_connected(&PEER_A));
        manager.add_connection(PEER_A, "127.0.0.1:9473".to_string());
        assert!(manager.is_connected(&PEER_A));
        manager.remove_connection(&PEER_A);
        assert!(!manager.is_connected(&PEER_A));
    }

    #[test]
    fn connected_peers_lists_all() {
        let mut manager = PeerManager::new();
        manager.add_connection(PEER_A, "1.1.1.1:9473".to_string());
        manager.add_connection(PEER_B, "2.2.2.2:9473".to_string());
        manager.add_connection(PEER_C, "3.3.3.3:9473".to_string());

        let mut peers = manager.connected_peers();
        peers.sort();
        let mut expected = vec![PEER_A, PEER_B, PEER_C];
        expected.sort();
        assert_eq!(peers, expected);
    }

    #[test]
    fn get_address_returns_correct_value() {
        let mut manager = PeerManager::new();
        manager.add_connection(PEER_A, "10.0.0.1:9473".to_string());
        assert_eq!(manager.get_address(&PEER_A).unwrap(), "10.0.0.1:9473");
        assert!(manager.get_address(&PEER_B).is_none());
    }

    #[test]
    fn connection_info_has_timestamp() {
        let mut manager = PeerManager::new();
        manager.add_connection(PEER_A, "127.0.0.1:9473".to_string());
        let info = manager.get_connection_info(&PEER_A).unwrap();
        assert!(info.connected_at > 0);
        assert_eq!(info.address, "127.0.0.1:9473");
    }

    #[test]
    fn multiple_add_remove_cycles() {
        let mut manager = PeerManager::new();

        manager.add_connection(PEER_A, "1.1.1.1:9473".to_string());
        assert_eq!(manager.connected_count(), 1);

        manager.remove_connection(&PEER_A);
        assert_eq!(manager.connected_count(), 0);

        let re_added = manager.add_connection(PEER_A, "2.2.2.2:9473".to_string());
        assert!(re_added);
        assert_eq!(manager.get_address(&PEER_A).unwrap(), "2.2.2.2:9473");
    }
}

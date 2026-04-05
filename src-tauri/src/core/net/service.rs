use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use snow::Keypair;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio::time;

use super::discovery::MdnsDiscovery;
use crate::net::actor::{PeerActor, PeerCommand};
use crate::net::event_sink::NetEventSink;
use crate::net::peer_manager::PeerManager;
use crate::net::transport::SecureConnection;
use crate::store::Store;
use crate::sync::lamport::LamportClock;
use crate::types::{PeerId, WireMessage};

const PEER_COMMAND_CHANNEL_SIZE: usize = 64;
const MDNS_POLL_INTERVAL: Duration = Duration::from_secs(30);
const RECONNECT_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug)]
pub enum NetworkCommand {
    SendMessage {
        peer_id: PeerId,
        message: WireMessage,
    },
    Connect {
        address: String,
    },
}

pub struct NetworkService {
    bind_address: String,
    local_peer_id: PeerId,
    noise_keypair: Keypair,
    signing_pk: [u8; 32],
    peer_manager: Arc<Mutex<PeerManager>>,
    store: Arc<Mutex<Store>>,
    lamport: Arc<Mutex<LamportClock>>,
    session_password: Arc<Mutex<Option<String>>>,
    command_rx: mpsc::Receiver<NetworkCommand>,
    peer_channels: HashMap<PeerId, mpsc::Sender<PeerCommand>>,
}

impl NetworkService {
    pub fn new(
        bind_address: String,
        local_peer_id: PeerId,
        noise_keypair: Keypair,
        signing_pk: [u8; 32],
        peer_manager: Arc<Mutex<PeerManager>>,
        store: Arc<Mutex<Store>>,
        lamport: Arc<Mutex<LamportClock>>,
        session_password: Arc<Mutex<Option<String>>>,
    ) -> (Self, mpsc::Sender<NetworkCommand>) {
        let (command_tx, command_rx) = mpsc::channel(64);

        let service = Self {
            bind_address,
            local_peer_id,
            noise_keypair,
            signing_pk,
            peer_manager,
            store,
            lamport,
            session_password,
            command_rx,
            peer_channels: HashMap::new(),
        };

        (service, command_tx)
    }

    pub async fn run(mut self, event_sink: Arc<dyn NetEventSink>) {
        let listener = match TcpListener::bind(&self.bind_address).await {
            Ok(listener) => {
                tracing::info!("network service listening on {}", self.bind_address);
                listener
            }
            Err(error) => {
                tracing::error!("failed to bind {}: {error}", self.bind_address);
                return;
            }
        };

        let listen_port = self.parse_listen_port();

        let mut mdns_discovery = match MdnsDiscovery::new(&self.local_peer_id, listen_port) {
            Ok(discovery) => {
                tracing::info!("mDNS discovery started");
                Some(discovery)
            }
            Err(error) => {
                tracing::warn!("mDNS initialization failed: {error}");
                None
            }
        };

        let mut mdns_poll_interval = time::interval(MDNS_POLL_INTERVAL);
        let mut reconnect_interval = time::interval(RECONNECT_INTERVAL);

        loop {
            tokio::select! {
                accept_result = listener.accept() => {
                    match accept_result {
                        Ok((stream, addr)) => {
                            tracing::debug!("incoming connection from {addr}");
                            self.handle_incoming_connection(
                                stream,
                                addr.to_string(),
                                event_sink.clone(),
                            ).await;
                        }
                        Err(error) => {
                            tracing::warn!("accept error: {error}");
                        }
                    }
                }
                command = self.command_rx.recv() => {
                    match command {
                        Some(cmd) => self.handle_command(cmd, event_sink.clone()).await,
                        None => {
                            tracing::info!("network command channel closed, shutting down");
                            break;
                        }
                    }
                }
                _ = mdns_poll_interval.tick() => {
                    if let Some(ref mut discovery) = mdns_discovery {
                        for peer in discovery.discovered_peers() {
                            if self.peer_channels.contains_key(&peer.peer_id) {
                                continue;
                            }
                            if let Some(address) = peer.addresses.first() {
                                tracing::info!(
                                    "mDNS: connecting to {} at {}",
                                    hex::encode(peer.peer_id),
                                    address
                                );
                                self.connect_to_peer(address, event_sink.clone()).await;
                            }
                        }
                    }
                }
                _ = reconnect_interval.tick() => {
                    self.reconnect_known_peers(event_sink.clone()).await;
                }
            }
        }

        if let Some(discovery) = mdns_discovery {
            if let Err(error) = discovery.shutdown() {
                tracing::warn!("mDNS shutdown error: {error}");
            }
        }
    }

    fn parse_listen_port(&self) -> u16 {
        self.bind_address
            .rsplit(':')
            .next()
            .and_then(|port_str| port_str.parse::<u16>().ok())
            .unwrap_or_else(|| {
                tracing::warn!(
                    "failed to parse port from '{}', using default 9473",
                    self.bind_address
                );
                9473
            })
    }

    async fn handle_incoming_connection(
        &mut self,
        stream: tokio::net::TcpStream,
        address: String,
        event_sink: Arc<dyn NetEventSink>,
    ) {
        let secure_connection = match SecureConnection::accept(
            stream,
            &self.noise_keypair,
            &self.local_peer_id,
            &self.signing_pk,
        )
        .await
        {
            Ok(connection) => connection,
            Err(error) => {
                tracing::warn!("handshake failed for incoming {address}: {error}");
                return;
            }
        };

        let remote_peer_id = *secure_connection.remote_peer_id();

        if self.peer_channels.contains_key(&remote_peer_id) {
            tracing::debug!(
                "duplicate connection from {}, dropping",
                hex::encode(remote_peer_id)
            );
            return;
        }

        self.register_peer(secure_connection, address, event_sink);
    }

    async fn handle_command(
        &mut self,
        command: NetworkCommand,
        event_sink: Arc<dyn NetEventSink>,
    ) {
        match command {
            NetworkCommand::SendMessage { peer_id, message } => {
                self.route_message(&peer_id, message).await;
            }
            NetworkCommand::Connect { address } => {
                self.connect_to_peer(&address, event_sink).await;
            }
        }
    }

    async fn route_message(&mut self, peer_id: &PeerId, message: WireMessage) {
        match self.peer_channels.get(peer_id) {
            Some(channel) => {
                if let Err(error) = channel.send(PeerCommand::Send(message)).await {
                    tracing::warn!(
                        "failed to route message to peer {}, removing stale channel: {error}",
                        hex::encode(peer_id)
                    );
                    self.peer_channels.remove(peer_id);
                }
            }
            None => {
                tracing::warn!(
                    "no connection to peer {} for message routing",
                    hex::encode(peer_id)
                );
            }
        }
    }

    async fn connect_to_peer(
        &mut self,
        address: &str,
        event_sink: Arc<dyn NetEventSink>,
    ) {
        let secure_connection = match SecureConnection::connect(
            address,
            &self.noise_keypair,
            &self.local_peer_id,
            &self.signing_pk,
        )
        .await
        {
            Ok(connection) => connection,
            Err(error) => {
                tracing::warn!("outbound connection to {address} failed: {error}");
                return;
            }
        };

        let remote_peer_id = *secure_connection.remote_peer_id();

        if self.peer_channels.contains_key(&remote_peer_id) {
            tracing::debug!(
                "already connected to {}, dropping outbound",
                hex::encode(remote_peer_id)
            );
            return;
        }

        self.register_peer(
            secure_connection,
            address.to_string(),
            event_sink,
        );
    }

    async fn reconnect_known_peers(&mut self, event_sink: Arc<dyn NetEventSink>) {
        let addresses = match self.store.lock() {
            Ok(store) => match store.get_all_peer_addresses() {
                Ok(addrs) => addrs,
                Err(error) => {
                    tracing::debug!("reconnect: failed to load peer addresses: {error}");
                    return;
                }
            },
            Err(error) => {
                tracing::debug!("reconnect: store lock poisoned: {error}");
                return;
            }
        };

        for peer_address in &addresses {
            if peer_address.peer_id == self.local_peer_id {
                continue;
            }
            if self.peer_channels.contains_key(&peer_address.peer_id) {
                continue;
            }
            tracing::debug!(
                "reconnect: attempting {} at {}",
                hex::encode(peer_address.peer_id),
                peer_address.address
            );
            self.connect_to_peer(&peer_address.address, event_sink.clone())
                .await;
        }
    }

    fn resolve_display_name(&self, peer_id: &PeerId) -> String {
        let store = match self.store.lock() {
            Ok(s) => s,
            Err(error) => {
                tracing::warn!("store lock poisoned during display_name resolve: {error}");
                return String::new();
            }
        };

        let chats = match store.list_chats() {
            Ok(c) => c,
            Err(error) => {
                tracing::warn!("failed to list chats for display_name resolve: {error}");
                return String::new();
            }
        };

        for chat in &chats {
            let members = match store.get_chat_members(&chat.chat_id) {
                Ok(m) => m,
                Err(error) => {
                    tracing::debug!(
                        "failed to get members for chat {}: {error}",
                        hex::encode(chat.chat_id)
                    );
                    continue;
                }
            };
            for member in &members {
                if &member.peer_id == peer_id && !member.display_name.is_empty() {
                    return member.display_name.clone();
                }
            }
        }

        String::new()
    }

    fn register_peer(
        &mut self,
        connection: SecureConnection,
        address: String,
        event_sink: Arc<dyn NetEventSink>,
    ) {
        let remote_peer_id = *connection.remote_peer_id();
        let display_name = self.resolve_display_name(&remote_peer_id);

        {
            let mut peer_manager = match self.peer_manager.lock() {
                Ok(pm) => pm,
                Err(error) => {
                    tracing::error!("peer_manager lock poisoned: {error}");
                    return;
                }
            };
            peer_manager.add_connection(remote_peer_id, address, display_name.clone());
        }

        event_sink.on_peer_connected(&remote_peer_id, &display_name);

        let (command_tx, command_rx) = mpsc::channel(PEER_COMMAND_CHANNEL_SIZE);
        self.peer_channels.insert(remote_peer_id, command_tx);

        let actor = PeerActor::new(
            connection,
            command_rx,
            self.store.clone(),
            self.lamport.clone(),
            self.peer_manager.clone(),
            self.session_password.clone(),
            event_sink,
        );

        tokio::spawn(async move {
            actor.run().await;
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::Store;
    use crate::sync::lamport::LamportClock;
    use crate::types::{Chat, ChatMember, MemberRole, WireMessage};

    fn test_peer_id() -> PeerId {
        [0xAA; 16]
    }

    fn test_service(store: Store) -> NetworkService {
        let noise_keypair = crate::crypto::noise::generate_noise_keypair().unwrap();
        let (service, _command_tx) = NetworkService::new(
            "127.0.0.1:0".to_string(),
            test_peer_id(),
            noise_keypair,
            [0u8; 32],
            Arc::new(Mutex::new(PeerManager::new())),
            Arc::new(Mutex::new(store)),
            Arc::new(Mutex::new(LamportClock::new())),
            Arc::new(Mutex::new(None)),
        );
        service
    }

    fn insert_chat_with_member(
        store: &Store,
        chat_id: [u8; 16],
        peer_id: PeerId,
        display_name: &str,
    ) {
        let chat = Chat {
            chat_id,
            chat_name: "test-chat".to_string(),
            owner_peer_id: test_peer_id(),
            created_at: 1000,
            my_lamport_counter: 0,
            unread_count: 0,
        };
        store.insert_chat(&chat).unwrap();

        let member = ChatMember {
            chat_id,
            peer_id,
            signing_pk: [0u8; 32],
            exchange_pk: [0u8; 32],
            display_name: display_name.to_string(),
            role: MemberRole::Member,
            added_at: 2000,
            added_by: test_peer_id(),
            is_removed: false,
        };
        store.insert_chat_member(&member).unwrap();
    }

    fn test_service_with_address(store: Store, address: &str) -> NetworkService {
        let noise_keypair = crate::crypto::noise::generate_noise_keypair().unwrap();
        let (service, _command_tx) = NetworkService::new(
            address.to_string(),
            test_peer_id(),
            noise_keypair,
            [0u8; 32],
            Arc::new(Mutex::new(PeerManager::new())),
            Arc::new(Mutex::new(store)),
            Arc::new(Mutex::new(LamportClock::new())),
            Arc::new(Mutex::new(None)),
        );
        service
    }

    // --- parse_listen_port ---

    #[test]
    fn parse_listen_port_extracts_valid_port() {
        let store = Store::open_in_memory().unwrap();
        let service = test_service_with_address(store, "127.0.0.1:9473");
        assert_eq!(service.parse_listen_port(), 9473);
    }

    #[test]
    fn parse_listen_port_handles_port_zero() {
        let store = Store::open_in_memory().unwrap();
        let service = test_service_with_address(store, "0.0.0.0:0");
        assert_eq!(service.parse_listen_port(), 0);
    }

    #[test]
    fn parse_listen_port_defaults_on_invalid_format() {
        let store = Store::open_in_memory().unwrap();
        let service = test_service_with_address(store, "invalid");
        assert_eq!(service.parse_listen_port(), 9473);
    }

    // --- resolve_display_name ---

    #[test]
    fn resolve_display_name_returns_name_when_member_exists() {
        let store = Store::open_in_memory().unwrap();
        let target_peer: PeerId = [0xBB; 16];
        insert_chat_with_member(&store, [1u8; 16], target_peer, "Alice");

        let service = test_service(store);
        let name = service.resolve_display_name(&target_peer);

        assert_eq!(name, "Alice");
    }

    #[test]
    fn resolve_display_name_returns_empty_when_peer_not_found() {
        let store = Store::open_in_memory().unwrap();
        let unknown_peer: PeerId = [0xCC; 16];

        let service = test_service(store);
        let name = service.resolve_display_name(&unknown_peer);

        assert!(name.is_empty());
    }

    #[test]
    fn resolve_display_name_returns_empty_when_no_chats() {
        let store = Store::open_in_memory().unwrap();

        let service = test_service(store);
        let name = service.resolve_display_name(&[0xBB; 16]);

        assert!(name.is_empty());
    }

    #[test]
    fn resolve_display_name_skips_empty_display_names() {
        let store = Store::open_in_memory().unwrap();
        let target_peer: PeerId = [0xBB; 16];
        insert_chat_with_member(&store, [1u8; 16], target_peer, "");

        let service = test_service(store);
        let name = service.resolve_display_name(&target_peer);

        assert!(name.is_empty());
    }

    #[test]
    fn resolve_display_name_returns_first_nonempty_across_chats() {
        let store = Store::open_in_memory().unwrap();
        let target_peer: PeerId = [0xBB; 16];

        // First chat: empty display name
        insert_chat_with_member(&store, [1u8; 16], target_peer, "");

        // Second chat: actual display name
        let chat_b = Chat {
            chat_id: [2u8; 16],
            chat_name: "other-chat".to_string(),
            owner_peer_id: test_peer_id(),
            created_at: 2000,
            my_lamport_counter: 0,
            unread_count: 0,
        };
        store.insert_chat(&chat_b).unwrap();
        let member_b = ChatMember {
            chat_id: [2u8; 16],
            peer_id: target_peer,
            signing_pk: [0u8; 32],
            exchange_pk: [0u8; 32],
            display_name: "Bob".to_string(),
            role: MemberRole::Member,
            added_at: 3000,
            added_by: test_peer_id(),
            is_removed: false,
        };
        store.insert_chat_member(&member_b).unwrap();

        let service = test_service(store);
        let name = service.resolve_display_name(&target_peer);

        assert_eq!(name, "Bob");
    }

    // --- route_message ---

    #[tokio::test]
    async fn route_message_sends_to_connected_peer() {
        let store = Store::open_in_memory().unwrap();
        let mut service = test_service(store);

        let target_peer: PeerId = [0xBB; 16];
        let (sender, mut receiver) = mpsc::channel(8);
        service.peer_channels.insert(target_peer, sender);

        let message = WireMessage::Ping { timestamp: 42 };
        service.route_message(&target_peer, message).await;

        let received = receiver.recv().await.unwrap();
        match received {
            PeerCommand::Send(WireMessage::Ping { timestamp }) => {
                assert_eq!(timestamp, 42);
            }
            _ => panic!("expected PeerCommand::Send(WireMessage::Ping)"),
        }
    }

    #[tokio::test]
    async fn route_message_logs_when_peer_not_connected() {
        let store = Store::open_in_memory().unwrap();
        let mut service = test_service(store);

        let unknown_peer: PeerId = [0xCC; 16];
        let message = WireMessage::Ping { timestamp: 1 };

        // Should not panic — just logs a warning
        service.route_message(&unknown_peer, message).await;

        assert!(!service.peer_channels.contains_key(&unknown_peer));
    }

    #[tokio::test]
    async fn route_message_removes_stale_channel_on_send_failure() {
        let store = Store::open_in_memory().unwrap();
        let mut service = test_service(store);

        let target_peer: PeerId = [0xBB; 16];
        let (sender, receiver) = mpsc::channel(8);
        service.peer_channels.insert(target_peer, sender);

        // Drop receiver to simulate dead peer actor
        drop(receiver);

        let message = WireMessage::Ping { timestamp: 99 };
        service.route_message(&target_peer, message).await;

        assert!(
            !service.peer_channels.contains_key(&target_peer),
            "stale channel must be removed after failed send"
        );
    }
}

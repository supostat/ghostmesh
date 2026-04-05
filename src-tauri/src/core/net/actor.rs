use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time;

use crate::net::event_sink::NetEventSink;
use crate::net::handler;
use crate::net::peer_manager::PeerManager;
use crate::net::transport::SecureConnection;
use crate::store::Store;
use crate::sync::engine::SyncEngine;
use crate::sync::lamport::LamportClock;
use crate::types::PeerId;

const SYNC_INTERVAL: Duration = Duration::from_secs(60);
const PEER_EXCHANGE_INTERVAL: Duration = Duration::from_secs(300);

pub enum PeerCommand {
    Send(crate::types::WireMessage),
}

pub struct PeerActor {
    connection: SecureConnection,
    command_rx: mpsc::Receiver<PeerCommand>,
    store: Arc<Mutex<Store>>,
    lamport: Arc<Mutex<LamportClock>>,
    peer_manager: Arc<Mutex<PeerManager>>,
    session_password: Arc<Mutex<Option<String>>>,
    event_sink: Arc<dyn NetEventSink>,
}

impl PeerActor {
    pub fn new(
        connection: SecureConnection,
        command_rx: mpsc::Receiver<PeerCommand>,
        store: Arc<Mutex<Store>>,
        lamport: Arc<Mutex<LamportClock>>,
        peer_manager: Arc<Mutex<PeerManager>>,
        session_password: Arc<Mutex<Option<String>>>,
        event_sink: Arc<dyn NetEventSink>,
    ) -> Self {
        Self {
            connection,
            command_rx,
            store,
            lamport,
            peer_manager,
            session_password,
            event_sink,
        }
    }

    pub fn remote_peer_id(&self) -> PeerId {
        *self.connection.remote_peer_id()
    }

    pub async fn run(mut self) {
        let remote_peer_id = *self.connection.remote_peer_id();
        tracing::info!(
            "peer actor started for {}",
            hex::encode(remote_peer_id)
        );

        let mut sync_interval = time::interval(SYNC_INTERVAL);
        let mut peer_exchange_interval = time::interval(PEER_EXCHANGE_INTERVAL);

        // Skip the first immediate tick — timers fire at t=0 by default
        sync_interval.tick().await;
        peer_exchange_interval.tick().await;

        loop {
            tokio::select! {
                receive_result = self.connection.receive() => {
                    match receive_result {
                        Ok(wire_message) => {
                            if let Err(error) = self.handle_incoming(&wire_message) {
                                tracing::warn!(
                                    "handler error for peer {}: {error}",
                                    hex::encode(remote_peer_id)
                                );
                            }
                        }
                        Err(error) => {
                            tracing::info!(
                                "peer {} disconnected: {error}",
                                hex::encode(remote_peer_id)
                            );
                            break;
                        }
                    }
                }
                command = self.command_rx.recv() => {
                    match command {
                        Some(PeerCommand::Send(wire_message)) => {
                            if let Err(error) = self.connection.send(&wire_message).await {
                                tracing::warn!(
                                    "send error to peer {}: {error}",
                                    hex::encode(remote_peer_id)
                                );
                                break;
                            }
                        }
                        None => {
                            tracing::info!(
                                "command channel closed for peer {}",
                                hex::encode(remote_peer_id)
                            );
                            break;
                        }
                    }
                }
                _ = sync_interval.tick() => {
                    self.run_periodic_sync().await;
                }
                _ = peer_exchange_interval.tick() => {
                    self.run_periodic_peer_exchange().await;
                }
            }
        }

        self.cleanup(remote_peer_id);
    }

    fn get_chat_ids(&self) -> Vec<[u8; 16]> {
        let store = match self.store.lock() {
            Ok(s) => s,
            Err(error) => {
                tracing::warn!("store lock error: {error}");
                return Vec::new();
            }
        };
        match store.list_chats() {
            Ok(chats) => chats.into_iter().map(|c| c.chat_id).collect(),
            Err(error) => {
                tracing::warn!("list_chats error: {error}");
                Vec::new()
            }
        }
    }

    async fn run_periodic_sync(&mut self) {
        let remote_peer_id = *self.connection.remote_peer_id();

        let chat_ids = self.get_chat_ids();
        if chat_ids.is_empty() {
            return;
        }

        for chat_id in &chat_ids {
            let request = {
                let store = match self.store.lock() {
                    Ok(s) => s,
                    Err(error) => {
                        tracing::warn!("periodic sync: store lock error for chat {}: {error}", hex::encode(chat_id));
                        continue;
                    }
                };
                match SyncEngine::prepare_sync_request(&store, chat_id) {
                    Ok(req) => req,
                    Err(error) => {
                        tracing::warn!(
                            "periodic sync: prepare_sync_request failed for chat {}: {error}",
                            hex::encode(chat_id)
                        );
                        continue;
                    }
                }
            };

            if let Err(error) = self.connection.send(&request).await {
                tracing::warn!(
                    "periodic sync: send failed to peer {} for chat {}: {error}",
                    hex::encode(remote_peer_id),
                    hex::encode(chat_id)
                );
            }
        }
    }

    async fn run_periodic_peer_exchange(&mut self) {
        let remote_peer_id = *self.connection.remote_peer_id();

        let chat_ids = self.get_chat_ids();
        if chat_ids.is_empty() {
            return;
        }

        for chat_id in &chat_ids {
            let message = {
                let store = match self.store.lock() {
                    Ok(s) => s,
                    Err(error) => {
                        tracing::warn!("periodic peer exchange: store lock error for chat {}: {error}", hex::encode(chat_id));
                        continue;
                    }
                };
                match handler::prepare_peer_exchange(&store, chat_id) {
                    Ok(Some(msg)) => msg,
                    Ok(None) => continue,
                    Err(error) => {
                        tracing::warn!(
                            "periodic peer exchange: prepare failed for chat {}: {error}",
                            hex::encode(chat_id)
                        );
                        continue;
                    }
                }
            };

            if let Err(error) = self.connection.send(&message).await {
                tracing::warn!(
                    "periodic peer exchange: send failed to peer {} for chat {}: {error}",
                    hex::encode(remote_peer_id),
                    hex::encode(chat_id)
                );
            }
        }
    }

    fn handle_incoming(
        &mut self,
        wire_message: &crate::types::WireMessage,
    ) -> Result<(), crate::types::CoreError> {
        let remote_peer_id = *self.connection.remote_peer_id();

        let password = self
            .session_password
            .lock()
            .map_err(|e| crate::types::CoreError::Net(format!("password lock error: {e}")))?
            .clone()
            .unwrap_or_default();

        let response = {
            let store = self
                .store
                .lock()
                .map_err(|e| crate::types::CoreError::Net(format!("store lock error: {e}")))?;
            let mut lamport = self
                .lamport
                .lock()
                .map_err(|e| crate::types::CoreError::Net(format!("lamport lock error: {e}")))?;

            handler::dispatch(
                wire_message,
                &remote_peer_id,
                &store,
                &mut lamport,
                &password,
                self.event_sink.as_ref(),
            )?
        };

        if let Some(response_message) = response {
            let connection = &mut self.connection;
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    connection.send(&response_message).await
                })
            })?;
        }

        Ok(())
    }

    fn cleanup(&self, remote_peer_id: PeerId) {
        if let Ok(mut peer_manager) = self.peer_manager.lock() {
            peer_manager.remove_connection(&remote_peer_id);
        }
        self.event_sink.on_peer_disconnected(&remote_peer_id, "");
    }
}

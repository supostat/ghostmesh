use std::sync::{Arc, Mutex};

use tokio::sync::mpsc;

use crate::net::event_sink::NetEventSink;
use crate::net::handler;
use crate::net::peer_manager::PeerManager;
use crate::net::transport::SecureConnection;
use crate::store::Store;
use crate::sync::lamport::LamportClock;
use crate::types::PeerId;

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
            }
        }

        self.cleanup(remote_peer_id);
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

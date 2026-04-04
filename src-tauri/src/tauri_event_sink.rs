use tauri::{AppHandle, Emitter};

use ghostmesh_core::net::NetEventSink;
use ghostmesh_core::types::{ChatId, MessageId, PeerId};

use crate::types::{
    ChatJoinComplete, DeliveryAck, NetworkStatus, PeerEvent, SyncComplete, SyncProgress,
};

pub struct TauriEventSink {
    app: AppHandle,
}

impl TauriEventSink {
    pub fn new(app: AppHandle) -> Self {
        Self { app }
    }
}

impl NetEventSink for TauriEventSink {
    fn on_peer_connected(&self, peer_id: &PeerId, display_name: &str) {
        let event = PeerEvent {
            peer_id: hex::encode(peer_id),
            display_name: display_name.to_string(),
        };
        if let Err(error) = self.app.emit("peer:connected", &event) {
            tracing::warn!("failed to emit peer:connected: {error}");
        }
    }

    fn on_peer_disconnected(&self, peer_id: &PeerId, display_name: &str) {
        let event = PeerEvent {
            peer_id: hex::encode(peer_id),
            display_name: display_name.to_string(),
        };
        if let Err(error) = self.app.emit("peer:disconnected", &event) {
            tracing::warn!("failed to emit peer:disconnected: {error}");
        }
    }

    fn on_sync_progress(
        &self,
        chat_id: &ChatId,
        _peer_id: &PeerId,
        received: u64,
        total: u64,
    ) {
        let event = SyncProgress {
            chat_id: hex::encode(chat_id),
            received,
            total,
        };
        if let Err(error) = self.app.emit("sync:progress", &event) {
            tracing::warn!("failed to emit sync:progress: {error}");
        }
    }

    fn on_sync_complete(&self, chat_id: &ChatId, new_messages: u64) {
        let event = SyncComplete {
            chat_id: hex::encode(chat_id),
            new_messages,
        };
        if let Err(error) = self.app.emit("sync:complete", &event) {
            tracing::warn!("failed to emit sync:complete: {error}");
        }
    }

    fn on_delivery_ack(&self, message_id: &MessageId, peer_id: &PeerId) {
        let event = DeliveryAck {
            message_id: hex::encode(message_id),
            peer_id: hex::encode(peer_id),
        };
        if let Err(error) = self.app.emit("delivery:ack", &event) {
            tracing::warn!("failed to emit delivery:ack: {error}");
        }
    }

    fn on_network_status(&self, connected_peers: u32, outbox_size: u32) {
        let event = NetworkStatus {
            connected_peers,
            outbox_size,
        };
        if let Err(error) = self.app.emit("network:status", &event) {
            tracing::warn!("failed to emit network:status: {error}");
        }
    }

    fn on_chat_join_complete(&self, chat_id: &ChatId, chat_name: &str) {
        let event = ChatJoinComplete {
            chat_id: hex::encode(chat_id),
            chat_name: chat_name.to_string(),
        };
        if let Err(error) = self.app.emit("chat:join_complete", &event) {
            tracing::warn!("failed to emit chat:join_complete: {error}");
        }
    }
}

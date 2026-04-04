use crate::types::{ChatId, MessageId, PeerId};

pub trait NetEventSink: Send + Sync {
    fn on_peer_connected(&self, peer_id: &PeerId, display_name: &str);
    fn on_peer_disconnected(&self, peer_id: &PeerId, display_name: &str);
    fn on_sync_progress(
        &self,
        chat_id: &ChatId,
        peer_id: &PeerId,
        received: u64,
        total: u64,
    );
    fn on_sync_complete(&self, chat_id: &ChatId, new_messages: u64);
    fn on_delivery_ack(&self, message_id: &MessageId, peer_id: &PeerId);
    fn on_network_status(&self, connected_peers: u32, outbox_size: u32);
    fn on_chat_join_complete(&self, chat_id: &ChatId, chat_name: &str);
}

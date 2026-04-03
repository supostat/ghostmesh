use tauri::{AppHandle, Emitter};

use crate::types::{
    ChatJoinComplete, DeliveryAck, MemberEvent, MessageInfo, NetworkStatus, PeerEvent,
    SyncComplete, SyncProgress,
};

pub fn emit_message_new(app: &AppHandle, message: &MessageInfo) -> Result<(), String> {
    app.emit("message:new", message)
        .map_err(|e| format!("failed to emit message:new: {e}"))
}

pub fn emit_peer_connected(app: &AppHandle, event: &PeerEvent) -> Result<(), String> {
    app.emit("peer:connected", event)
        .map_err(|e| format!("failed to emit peer:connected: {e}"))
}

pub fn emit_peer_disconnected(app: &AppHandle, event: &PeerEvent) -> Result<(), String> {
    app.emit("peer:disconnected", event)
        .map_err(|e| format!("failed to emit peer:disconnected: {e}"))
}

pub fn emit_sync_progress(app: &AppHandle, progress: &SyncProgress) -> Result<(), String> {
    app.emit("sync:progress", progress)
        .map_err(|e| format!("failed to emit sync:progress: {e}"))
}

pub fn emit_sync_complete(app: &AppHandle, complete: &SyncComplete) -> Result<(), String> {
    app.emit("sync:complete", complete)
        .map_err(|e| format!("failed to emit sync:complete: {e}"))
}

pub fn emit_delivery_ack(app: &AppHandle, ack: &DeliveryAck) -> Result<(), String> {
    app.emit("delivery:ack", ack)
        .map_err(|e| format!("failed to emit delivery:ack: {e}"))
}

pub fn emit_network_status(app: &AppHandle, status: &NetworkStatus) -> Result<(), String> {
    app.emit("network:status", status)
        .map_err(|e| format!("failed to emit network:status: {e}"))
}

pub fn emit_chat_member_joined(app: &AppHandle, event: &MemberEvent) -> Result<(), String> {
    app.emit("chat:member_joined", event)
        .map_err(|e| format!("failed to emit chat:member_joined: {e}"))
}

pub fn emit_chat_member_left(app: &AppHandle, event: &MemberEvent) -> Result<(), String> {
    app.emit("chat:member_left", event)
        .map_err(|e| format!("failed to emit chat:member_left: {e}"))
}

pub fn emit_chat_join_complete(
    app: &AppHandle,
    event: &ChatJoinComplete,
) -> Result<(), String> {
    app.emit("chat:join_complete", event)
        .map_err(|e| format!("failed to emit chat:join_complete: {e}"))
}

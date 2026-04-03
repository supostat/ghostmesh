import { invoke } from "@tauri-apps/api/core";

// --- IPC DTO Types ---

export interface IdentityInfo {
  peer_id: string;
  display_name: string;
  fingerprint: string;
  created_at: number;
}

export interface ChatInfo {
  chat_id: string;
  chat_name: string;
  owner_peer_id: string;
  created_at: number;
  member_count: number;
  online_count: number;
  last_message_preview?: string;
  last_message_at?: number;
  unread_count: number;
  pending_key_exchange: boolean;
}

export interface ChatDetail {
  chat_id: string;
  chat_name: string;
  owner_peer_id: string;
  created_at: number;
  members: MemberInfo[];
  key_epoch: number;
}

export interface MemberInfo {
  peer_id: string;
  display_name: string;
  fingerprint: string;
  role: string;
  is_online: boolean;
  added_at: number;
}

export interface MessageInfo {
  message_id: string;
  chat_id: string;
  author_peer_id: string;
  author_name: string;
  lamport_ts: number;
  created_at: number;
  text: string;
  delivery_status: DeliveryStatus;
}

export type DeliveryStatus =
  | "queued"
  | { partial: { delivered: number; total: number } }
  | "all";

export interface MessagePacket {
  message_id: string;
  chat_id: string;
  author_peer_id: string;
  lamport_ts: number;
  created_at: number;
  key_epoch: number;
  parent_ids: string[];
  signature: string;
  payload_size: number;
  text: string;
  delivery_acks: string[];
}

export interface InviteCode {
  code: string;
}

export interface PeerInfo {
  peer_id: string;
  display_name: string;
  addresses: string[];
  last_seen?: number;
  is_connected: boolean;
}

export interface ConnectionInfo {
  peer_id: string;
  display_name: string;
  address: string;
  connected_at: number;
  messages_synced: number;
}

export interface OutboxInfo {
  message_id: string;
  target_peer_id: string;
  chat_id: string;
  created_at: number;
}

export interface SyncLogInfo {
  id: number;
  timestamp: number;
  peer_id?: string;
  event_type: string;
  detail?: string;
}

export interface NetworkStatus {
  connected_peers: number;
  outbox_size: number;
}

export interface PeerEvent {
  peer_id: string;
  display_name: string;
}

export interface SyncProgress {
  chat_id: string;
  received: number;
  total: number;
}

export interface SyncComplete {
  chat_id: string;
  new_messages: number;
}

export interface DeliveryAck {
  message_id: string;
  peer_id: string;
}

export interface MemberEvent {
  chat_id: string;
  peer_id: string;
  display_name: string;
}

export interface ChatJoinComplete {
  chat_id: string;
  chat_name: string;
}

export interface Settings {
  display_name: string;
  listen_port: number;
  mdns_enabled: boolean;
  message_ttl_days?: number;
  theme: string;
  auto_update_enabled: boolean;
}

export interface UpdateInfo {
  version: string;
  body: string | null;
  date: string | null;
}

// --- Identity Commands ---

export function createIdentity(
  displayName: string,
  password: string,
): Promise<IdentityInfo> {
  return invoke("create_identity", { displayName, password });
}

export function getIdentity(): Promise<IdentityInfo> {
  return invoke("get_identity");
}

export function validatePassword(password: string): Promise<boolean> {
  return invoke("validate_password", { password });
}

export function exportIdentity(password: string): Promise<number[]> {
  return invoke("export_identity", { password });
}

export function importIdentity(
  encryptedExport: number[],
  password: string,
): Promise<IdentityInfo> {
  return invoke("import_identity", { encryptedExport, password });
}

// --- Chat Commands ---

export function createChat(
  chatName: string,
  password: string,
): Promise<ChatInfo> {
  return invoke("create_chat", { chatName, password });
}

export function listChats(): Promise<ChatInfo[]> {
  return invoke("list_chats");
}

export function getChat(chatId: string): Promise<ChatDetail> {
  return invoke("get_chat", { chatId });
}

export function generateInvite(chatId: string): Promise<InviteCode> {
  return invoke("generate_invite", { chatId });
}

export function joinChat(inviteCode: string): Promise<ChatInfo> {
  return invoke("join_chat", { inviteCode });
}

export function leaveChat(chatId: string): Promise<void> {
  return invoke("leave_chat", { chatId });
}

// --- Message Commands ---

export function sendMessage(
  chatId: string,
  text: string,
  password: string,
): Promise<MessageInfo> {
  return invoke("send_message", { chatId, text, password });
}

export function getMessages(
  chatId: string,
  password: string,
  beforeLamport?: number,
  limit?: number,
): Promise<MessageInfo[]> {
  return invoke("get_messages", { chatId, password, beforeLamport, limit });
}

export function getMessageDetail(
  messageId: string,
  password: string,
): Promise<MessagePacket> {
  return invoke("get_message_detail", { messageId, password });
}

// --- Network Commands ---

export function getPeers(): Promise<PeerInfo[]> {
  return invoke("get_peers");
}

export function getConnections(): Promise<ConnectionInfo[]> {
  return invoke("get_connections");
}

export function getOutbox(): Promise<OutboxInfo[]> {
  return invoke("get_outbox");
}

export function addManualPeer(
  peerId: string,
  address: string,
): Promise<void> {
  return invoke("add_manual_peer", { peerId, address });
}

export function getSyncLog(limit?: number): Promise<SyncLogInfo[]> {
  return invoke("get_sync_log", { limit });
}

// --- Settings Commands ---

export function getSettings(): Promise<Settings> {
  return invoke("get_settings");
}

export function updateSettings(newSettings: Settings): Promise<Settings> {
  return invoke("update_settings", { newSettings });
}

// --- Update Commands ---

export function checkForUpdate(): Promise<UpdateInfo | null> {
  return invoke("check_for_update");
}

export function installUpdate(): Promise<void> {
  return invoke("install_update");
}

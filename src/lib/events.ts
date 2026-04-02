import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  MessageInfo,
  PeerEvent,
  SyncProgress,
  SyncComplete,
  DeliveryAck,
  NetworkStatus,
  MemberEvent,
} from "./api";

export function onMessageNew(
  callback: (message: MessageInfo) => void,
): Promise<UnlistenFn> {
  return listen("message:new", (event) =>
    callback(event.payload as MessageInfo),
  );
}

export function onPeerConnected(
  callback: (event: PeerEvent) => void,
): Promise<UnlistenFn> {
  return listen("peer:connected", (event) =>
    callback(event.payload as PeerEvent),
  );
}

export function onPeerDisconnected(
  callback: (event: PeerEvent) => void,
): Promise<UnlistenFn> {
  return listen("peer:disconnected", (event) =>
    callback(event.payload as PeerEvent),
  );
}

export function onSyncProgress(
  callback: (progress: SyncProgress) => void,
): Promise<UnlistenFn> {
  return listen("sync:progress", (event) =>
    callback(event.payload as SyncProgress),
  );
}

export function onSyncComplete(
  callback: (complete: SyncComplete) => void,
): Promise<UnlistenFn> {
  return listen("sync:complete", (event) =>
    callback(event.payload as SyncComplete),
  );
}

export function onDeliveryAck(
  callback: (ack: DeliveryAck) => void,
): Promise<UnlistenFn> {
  return listen("delivery:ack", (event) =>
    callback(event.payload as DeliveryAck),
  );
}

export function onNetworkStatus(
  callback: (status: NetworkStatus) => void,
): Promise<UnlistenFn> {
  return listen("network:status", (event) =>
    callback(event.payload as NetworkStatus),
  );
}

export function onChatMemberJoined(
  callback: (event: MemberEvent) => void,
): Promise<UnlistenFn> {
  return listen("chat:member_joined", (event) =>
    callback(event.payload as MemberEvent),
  );
}

export function onChatMemberLeft(
  callback: (event: MemberEvent) => void,
): Promise<UnlistenFn> {
  return listen("chat:member_left", (event) =>
    callback(event.payload as MemberEvent),
  );
}

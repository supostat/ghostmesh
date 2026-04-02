import {
  getPeers,
  getConnections,
  getOutbox,
  getSyncLog,
  type PeerInfo,
  type ConnectionInfo,
  type OutboxInfo,
  type SyncLogInfo,
  type NetworkStatus,
} from "../api";

let peers = $state<PeerInfo[]>([]);
let connections = $state<ConnectionInfo[]>([]);
let outbox = $state<OutboxInfo[]>([]);
let syncLog = $state<SyncLogInfo[]>([]);
let networkStatus = $state<NetworkStatus>({
  connected_peers: 0,
  outbox_size: 0,
});
let loading = $state(false);
let error = $state<string | null>(null);

export function getPeerList() {
  return peers;
}

export function getConnectionList() {
  return connections;
}

export function getOutboxList() {
  return outbox;
}

export function getSyncLogList() {
  return syncLog;
}

export function getNetworkStatus() {
  return networkStatus;
}

export function isNetworkLoading() {
  return loading;
}

export function getNetworkError() {
  return error;
}

export async function loadPeers(): Promise<void> {
  try {
    peers = await getPeers();
  } catch (err) {
    error = String(err);
  }
}

export async function loadConnections(): Promise<void> {
  try {
    connections = await getConnections();
  } catch (err) {
    error = String(err);
  }
}

export async function loadOutbox(): Promise<void> {
  try {
    outbox = await getOutbox();
  } catch (err) {
    error = String(err);
  }
}

export async function loadSyncLog(limit?: number): Promise<void> {
  try {
    syncLog = await getSyncLog(limit);
  } catch (err) {
    error = String(err);
  }
}

export async function loadAllNetworkData(): Promise<void> {
  loading = true;
  error = null;
  try {
    const [peersData, connectionsData, outboxData, syncLogData] =
      await Promise.all([
        getPeers(),
        getConnections(),
        getOutbox(),
        getSyncLog(50),
      ]);
    peers = peersData;
    connections = connectionsData;
    outbox = outboxData;
    syncLog = syncLogData;
  } catch (err) {
    error = String(err);
  } finally {
    loading = false;
  }
}

export function setNetworkStatus(status: NetworkStatus): void {
  networkStatus = status;
}

export function addPeerConnection(peerId: string, _displayName: string): void {
  const index = peers.findIndex((p) => p.peer_id === peerId);
  if (index >= 0) {
    peers[index] = { ...peers[index], is_connected: true };
  }
  networkStatus = {
    ...networkStatus,
    connected_peers: networkStatus.connected_peers + 1,
  };
}

export function removePeerConnection(peerId: string): void {
  const index = peers.findIndex((p) => p.peer_id === peerId);
  if (index >= 0) {
    peers[index] = { ...peers[index], is_connected: false };
  }
  networkStatus = {
    ...networkStatus,
    connected_peers: Math.max(0, networkStatus.connected_peers - 1),
  };
}

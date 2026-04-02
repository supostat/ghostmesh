<script lang="ts">
  import PeerIndicator from "./PeerIndicator.svelte";
  import {
    getPeerList,
    getConnectionList,
    getOutboxList,
    getSyncLogList,
    getNetworkStatus,
    loadAllNetworkData,
  } from "../lib/stores/network.svelte";

  let { onClose }: { onClose: () => void } = $props();

  let peers = $derived(getPeerList());
  let connections = $derived(getConnectionList());
  let outbox = $derived(getOutboxList());
  let syncLog = $derived(getSyncLogList());
  let networkStatus = $derived(getNetworkStatus());

  $effect(() => {
    loadAllNetworkData();
  });

  function formatTimestamp(timestampSeconds: number): string {
    return new Date(timestampSeconds * 1000).toLocaleString();
  }

  function shortId(id: string): string {
    return id.slice(0, 12);
  }

  function handleRefresh() {
    loadAllNetworkData();
  }
</script>

<div class="network-dashboard">
  <div class="dashboard-header">
    <h2>Network Dashboard</h2>
    <div class="header-actions">
      <button onclick={handleRefresh}>Refresh</button>
      <button class="close-btn" onclick={onClose}>&times;</button>
    </div>
  </div>

  <div class="dashboard-content">
    <div class="stats-bar">
      <div class="stat-item">
        <span class="stat-value mono">{networkStatus.connected_peers}</span>
        <span class="stat-label">Connected</span>
      </div>
      <div class="stat-item">
        <span class="stat-value mono">{peers.length}</span>
        <span class="stat-label">Known Peers</span>
      </div>
      <div class="stat-item">
        <span class="stat-value mono">{outbox.length}</span>
        <span class="stat-label">Outbox</span>
      </div>
    </div>

    <section class="section">
      <h3 class="section-title">Active Connections</h3>
      {#if connections.length === 0}
        <p class="text-muted text-small">No active connections</p>
      {:else}
        <div class="connection-list">
          {#each connections as connection (connection.peer_id)}
            <div class="connection-row">
              <PeerIndicator connected={true} />
              <div class="connection-info">
                <span class="mono text-small selectable">{shortId(connection.peer_id)}</span>
                <span class="text-muted text-small">{connection.address}</span>
              </div>
              <span class="mono text-muted text-small">{formatTimestamp(connection.connected_at)}</span>
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <section class="section">
      <h3 class="section-title">Known Peers</h3>
      {#if peers.length === 0}
        <p class="text-muted text-small">No known peers</p>
      {:else}
        <div class="peer-list">
          {#each peers as peer (peer.peer_id)}
            <div class="peer-row">
              <PeerIndicator connected={peer.is_connected} />
              <div class="peer-info">
                <span class="mono text-small selectable">{shortId(peer.peer_id)}</span>
                {#each peer.addresses as address}
                  <span class="text-muted text-small">{address}</span>
                {/each}
              </div>
              {#if peer.last_seen}
                <span class="mono text-muted text-small">{formatTimestamp(peer.last_seen)}</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <section class="section">
      <h3 class="section-title">Outbox ({outbox.length})</h3>
      {#if outbox.length === 0}
        <p class="text-muted text-small">All messages delivered</p>
      {:else}
        <div class="outbox-list">
          {#each outbox as entry (entry.message_id + entry.target_peer_id)}
            <div class="outbox-row">
              <span class="mono text-small selectable">{shortId(entry.message_id)}</span>
              <span class="text-secondary text-small">&rarr;</span>
              <span class="mono text-small selectable">{shortId(entry.target_peer_id)}</span>
            </div>
          {/each}
        </div>
      {/if}
    </section>

    <section class="section">
      <h3 class="section-title">Sync Log</h3>
      {#if syncLog.length === 0}
        <p class="text-muted text-small">No sync events</p>
      {:else}
        <div class="sync-log">
          {#each syncLog as entry (entry.id)}
            <div class="log-row">
              <span class="mono text-muted text-small">{formatTimestamp(entry.timestamp)}</span>
              <span class="badge badge-muted">{entry.event_type}</span>
              {#if entry.peer_id}
                <span class="mono text-small">{shortId(entry.peer_id)}</span>
              {/if}
              {#if entry.detail}
                <span class="text-secondary text-small">{entry.detail}</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    </section>
  </div>
</div>

<style>
  .network-dashboard {
    display: flex;
    flex-direction: column;
    height: 100%;
    flex: 1;
    min-width: 0;
  }

  .dashboard-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 16px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-secondary);
    height: var(--header-height);
    flex-shrink: 0;
  }

  .dashboard-header h2 {
    font-size: 15px;
    font-weight: 600;
  }

  .header-actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .close-btn {
    background: none;
    border: none;
    padding: 4px 8px;
    font-size: 20px;
    color: var(--text-secondary);
    cursor: pointer;
  }

  .close-btn:hover {
    color: var(--text-primary);
  }

  .dashboard-content {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 20px;
  }

  .stats-bar {
    display: flex;
    gap: 16px;
  }

  .stat-item {
    display: flex;
    flex-direction: column;
    align-items: center;
    padding: 12px 20px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius);
    flex: 1;
  }

  .stat-value {
    font-size: 24px;
    font-weight: 700;
    color: var(--accent);
  }

  .stat-label {
    font-size: 12px;
    color: var(--text-secondary);
    margin-top: 4px;
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .section-title {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-secondary);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .connection-list, .peer-list, .outbox-list, .sync-log {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .connection-row, .peer-row, .outbox-row, .log-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
    background: var(--bg-secondary);
    border-radius: var(--radius);
  }

  .connection-info, .peer-info {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }
</style>

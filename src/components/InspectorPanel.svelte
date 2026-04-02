<script lang="ts">
  import { getMessageDetail, type MessagePacket } from "../lib/api";
  import { getSelectedChatDetail } from "../lib/stores/chats.svelte";
  import { getOutboxList } from "../lib/stores/network.svelte";
  import PeerIndicator from "./PeerIndicator.svelte";

  let {
    selectedMessageId,
    password,
    chatId,
    onClose,
  }: {
    selectedMessageId: string | null;
    password: string;
    chatId: string;
    onClose: () => void;
  } = $props();

  let messagePacket = $state<MessagePacket | null>(null);
  let packetLoading = $state(false);
  let packetError = $state<string | null>(null);
  let activeTab = $state<"message" | "members" | "outbox">("members");

  let chatDetail = $derived(getSelectedChatDetail());
  let allOutbox = $derived(getOutboxList());
  let chatOutbox = $derived(allOutbox.filter((o) => o.chat_id === chatId));

  $effect(() => {
    if (selectedMessageId) {
      activeTab = "message";
      loadPacket(selectedMessageId);
    }
  });

  async function loadPacket(messageId: string) {
    packetLoading = true;
    packetError = null;
    try {
      messagePacket = await getMessageDetail(messageId, password);
    } catch (err) {
      packetError = String(err);
      messagePacket = null;
    } finally {
      packetLoading = false;
    }
  }

  function formatTimestamp(timestampSeconds: number): string {
    return new Date(timestampSeconds * 1000).toLocaleString();
  }

  function shortId(id: string): string {
    return id.slice(0, 16);
  }
</script>

<aside class="inspector">
  <div class="inspector-header">
    <div class="tab-bar">
      <button
        class="tab"
        class:active={activeTab === "message"}
        onclick={() => (activeTab = "message")}
        disabled={!selectedMessageId}
      >
        Packet
      </button>
      <button
        class="tab"
        class:active={activeTab === "members"}
        onclick={() => (activeTab = "members")}
      >
        Members
      </button>
      <button
        class="tab"
        class:active={activeTab === "outbox"}
        onclick={() => (activeTab = "outbox")}
      >
        Outbox ({chatOutbox.length})
      </button>
    </div>
    <button class="close-btn" onclick={onClose}>&times;</button>
  </div>

  <div class="inspector-body">
    {#if activeTab === "message"}
      {#if packetLoading}
        <p class="text-muted">Loading packet...</p>
      {:else if packetError}
        <p class="text-error text-small">{packetError}</p>
      {:else if messagePacket}
        <div class="detail-group">
          <div class="detail-row">
            <span class="detail-label">Message ID</span>
            <span class="detail-value mono selectable">{shortId(messagePacket.message_id)}</span>
          </div>
          <div class="detail-row">
            <span class="detail-label">Author</span>
            <span class="detail-value mono selectable">{shortId(messagePacket.author_peer_id)}</span>
          </div>
          <div class="detail-row">
            <span class="detail-label">Lamport</span>
            <span class="detail-value mono">{messagePacket.lamport_ts}</span>
          </div>
          <div class="detail-row">
            <span class="detail-label">Created</span>
            <span class="detail-value mono">{formatTimestamp(messagePacket.created_at)}</span>
          </div>
          <div class="detail-row">
            <span class="detail-label">Key Epoch</span>
            <span class="detail-value mono">{messagePacket.key_epoch}</span>
          </div>
          <div class="detail-row">
            <span class="detail-label">Payload Size</span>
            <span class="detail-value mono">{messagePacket.payload_size} bytes</span>
          </div>
          <div class="detail-row">
            <span class="detail-label">Signature</span>
            <span class="detail-value mono selectable">{shortId(messagePacket.signature)}</span>
          </div>
          {#if messagePacket.parent_ids.length > 0}
            <div class="detail-row">
              <span class="detail-label">Parents</span>
              <span class="detail-value mono selectable">
                {messagePacket.parent_ids.map(shortId).join(", ")}
              </span>
            </div>
          {/if}
          {#if messagePacket.delivery_acks.length > 0}
            <div class="detail-row">
              <span class="detail-label">Acks</span>
              <span class="detail-value mono">{messagePacket.delivery_acks.length}</span>
            </div>
          {/if}
        </div>
      {:else}
        <p class="text-muted">Select a message to inspect</p>
      {/if}

    {:else if activeTab === "members"}
      {#if chatDetail}
        <div class="member-list">
          {#each chatDetail.members as member (member.peer_id)}
            <div class="member-row">
              <PeerIndicator connected={member.is_online} />
              <div class="member-info">
                <span class="member-name">{member.display_name || shortId(member.peer_id)}</span>
                <span class="member-meta mono text-muted">
                  {member.fingerprint.slice(0, 8)} &middot; {member.role}
                </span>
              </div>
            </div>
          {/each}
        </div>
        <div class="detail-row" style="margin-top: 12px;">
          <span class="detail-label">Key Epoch</span>
          <span class="detail-value mono">{chatDetail.key_epoch}</span>
        </div>
      {:else}
        <p class="text-muted">Loading chat details...</p>
      {/if}

    {:else if activeTab === "outbox"}
      {#if chatOutbox.length === 0}
        <p class="text-muted">Outbox empty for this chat</p>
      {:else}
        <div class="outbox-list">
          {#each chatOutbox as entry (entry.message_id + entry.target_peer_id)}
            <div class="outbox-row">
              <span class="mono text-small selectable">{shortId(entry.message_id)}</span>
              <span class="text-secondary text-small">&rarr;</span>
              <span class="mono text-small selectable">{shortId(entry.target_peer_id)}</span>
              <span class="mono text-muted text-small">{formatTimestamp(entry.created_at)}</span>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  </div>
</aside>

<style>
  .inspector {
    width: var(--inspector-width);
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg-secondary);
    border-left: 1px solid var(--border);
    flex-shrink: 0;
  }

  .inspector-header {
    display: flex;
    align-items: center;
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }

  .tab-bar {
    display: flex;
    flex: 1;
  }

  .tab {
    flex: 1;
    padding: 10px 8px;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    border-radius: 0;
    font-size: 12px;
    color: var(--text-secondary);
    cursor: pointer;
    transition: color 0.1s, border-color 0.1s;
  }

  .tab:hover:not(:disabled) {
    color: var(--text-primary);
    background: var(--bg-hover);
  }

  .tab.active {
    color: var(--accent);
    border-bottom-color: var(--accent);
  }

  .tab:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .close-btn {
    background: none;
    border: none;
    padding: 8px 12px;
    font-size: 20px;
    color: var(--text-secondary);
    cursor: pointer;
  }

  .close-btn:hover {
    color: var(--text-primary);
  }

  .inspector-body {
    flex: 1;
    overflow-y: auto;
    padding: 12px;
  }

  .detail-group {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .detail-row {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .detail-label {
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .detail-value {
    font-size: 12px;
    word-break: break-all;
  }

  .member-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .member-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px;
    border-radius: var(--radius);
  }

  .member-row:hover {
    background: var(--bg-hover);
  }

  .member-info {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .member-name {
    font-size: 13px;
    font-weight: 500;
  }

  .member-meta {
    font-size: 11px;
  }

  .outbox-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .outbox-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 6px;
    border-radius: var(--radius);
    background: var(--bg-primary);
  }
</style>

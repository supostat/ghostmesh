<script lang="ts">
  import type { ChatInfo } from "../lib/api";
  import PeerIndicator from "./PeerIndicator.svelte";
  import {
    getChatList,
    getSelectedChatId,
  } from "../lib/stores/chats.svelte";
  import { getNetworkStatus } from "../lib/stores/network.svelte";

  let {
    onSelectChat,
    onCreateChat,
    onJoinChat,
    onOpenSettings,
    onOpenNetwork,
  }: {
    onSelectChat: (chatId: string) => void;
    onCreateChat: () => void;
    onJoinChat: () => void;
    onOpenSettings: () => void;
    onOpenNetwork: () => void;
  } = $props();

  let chats = $derived(getChatList());
  let selectedChatId = $derived(getSelectedChatId());
  let networkStatus = $derived(getNetworkStatus());

  function formatLastMessage(chat: ChatInfo): string {
    if (chat.last_message_preview) {
      return chat.last_message_preview;
    }
    return "No messages yet";
  }

  function formatTimestamp(timestampSeconds: number | undefined): string {
    if (!timestampSeconds) return "";
    const date = new Date(timestampSeconds * 1000);
    const now = new Date();
    if (date.toDateString() === now.toDateString()) {
      return `${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
    }
    return `${String(date.getDate()).padStart(2, "0")}.${String(date.getMonth() + 1).padStart(2, "0")}`;
  }
</script>

<aside class="sidebar">
  <div class="sidebar-header">
    <h2 class="app-title">GhostMesh</h2>
    <div class="header-actions">
      <button class="icon-btn" title="Network" onclick={onOpenNetwork}>
        <PeerIndicator connected={networkStatus.connected_peers > 0} count={networkStatus.connected_peers} />
      </button>
      <button class="icon-btn" title="Settings" onclick={onOpenSettings}>
        &#9881;
      </button>
    </div>
  </div>

  <div class="chat-actions">
    <button class="primary" onclick={onCreateChat}>New Chat</button>
    <button onclick={onJoinChat}>Join</button>
  </div>

  <div class="chat-list">
    {#each chats as chat (chat.chat_id)}
      <button
        class="chat-item"
        class:active={selectedChatId === chat.chat_id}
        onclick={() => onSelectChat(chat.chat_id)}
      >
        <div class="chat-item-top">
          <span class="chat-name truncate">{chat.chat_name}</span>
          <span class="chat-time mono">{formatTimestamp(chat.last_message_at)}</span>
        </div>
        <div class="chat-item-bottom">
          <span class="chat-preview truncate text-secondary">{formatLastMessage(chat)}</span>
          <span class="chat-meta mono">
            <span class="member-count">{chat.online_count}/{chat.member_count}</span>
            {#if chat.unread_count > 0}
              <span class="unread-badge">{chat.unread_count}</span>
            {/if}
          </span>
        </div>
      </button>
    {:else}
      <div class="empty-state">
        <p class="text-muted">No chats yet</p>
        <p class="text-muted text-small">Create or join a chat to begin</p>
      </div>
    {/each}
  </div>

  <div class="sidebar-footer">
    <span class="text-muted text-small mono">
      Outbox: {networkStatus.outbox_size}
    </span>
  </div>
</aside>

<style>
  .sidebar {
    width: var(--sidebar-width);
    height: 100%;
    display: flex;
    flex-direction: column;
    background: var(--bg-secondary);
    border-right: 1px solid var(--border);
    flex-shrink: 0;
  }

  .sidebar-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px;
    border-bottom: 1px solid var(--border);
  }

  .app-title {
    font-size: 16px;
    font-weight: 700;
    color: var(--accent);
    font-family: var(--font-mono);
  }

  .header-actions {
    display: flex;
    gap: 4px;
    align-items: center;
  }

  .icon-btn {
    background: none;
    border: none;
    padding: 4px 8px;
    font-size: 16px;
    color: var(--text-secondary);
    cursor: pointer;
  }

  .icon-btn:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
    border-radius: var(--radius);
  }

  .chat-actions {
    display: flex;
    gap: 8px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--border);
  }

  .chat-actions button {
    flex: 1;
  }

  .chat-list {
    flex: 1;
    overflow-y: auto;
  }

  .chat-item {
    width: 100%;
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 10px 12px;
    border: none;
    border-radius: 0;
    border-bottom: 1px solid var(--border);
    background: transparent;
    text-align: left;
    cursor: pointer;
    transition: background 0.1s;
  }

  .chat-item:hover {
    background: var(--bg-hover);
  }

  .chat-item.active {
    background: var(--bg-active);
    border-left: 2px solid var(--accent);
  }

  .chat-item-top {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 8px;
  }

  .chat-name {
    font-weight: 600;
    font-size: 14px;
  }

  .chat-time {
    font-size: 11px;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .chat-item-bottom {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 8px;
  }

  .chat-preview {
    font-size: 13px;
    flex: 1;
  }

  .chat-meta {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .member-count {
    font-size: 11px;
    color: var(--text-muted);
  }

  .unread-badge {
    background: var(--accent);
    color: var(--bg-primary);
    font-size: 11px;
    font-weight: 700;
    padding: 1px 6px;
    border-radius: 10px;
    min-width: 18px;
    text-align: center;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    padding: 32px 16px;
    gap: 4px;
  }

  .sidebar-footer {
    padding: 8px 12px;
    border-top: 1px solid var(--border);
    text-align: center;
  }
</style>

<script lang="ts">
  import MessageRow from "./MessageRow.svelte";
  import { sendMessage } from "../lib/api";
  import { getMessageList, isMessagesLoading, appendMessage } from "../lib/stores/messages.svelte";
  import { getSelectedChatDetail } from "../lib/stores/chats.svelte";
  import { getIdentityState } from "../lib/stores/identity.svelte";

  let {
    chatId,
    password,
    onSelectMessage,
    onToggleInspector,
    onGenerateInvite,
  }: {
    chatId: string;
    password: string;
    onSelectMessage: (messageId: string) => void;
    onToggleInspector: () => void;
    onGenerateInvite: () => void;
  } = $props();

  let inputText = $state("");
  let sending = $state(false);
  let sendError = $state<string | null>(null);
  let messageListElement: HTMLDivElement | undefined = $state();

  let messages = $derived(getMessageList());
  let loading = $derived(isMessagesLoading());
  let chatDetail = $derived(getSelectedChatDetail());
  let identity = $derived(getIdentityState());

  let sortedMessages = $derived(
    [...messages].sort((a, b) => {
      if (a.lamport_ts !== b.lamport_ts) return a.lamport_ts - b.lamport_ts;
      return a.author_peer_id.localeCompare(b.author_peer_id);
    }),
  );

  $effect(() => {
    if (sortedMessages.length && messageListElement) {
      requestAnimationFrame(() => {
        messageListElement?.scrollTo({
          top: messageListElement.scrollHeight,
          behavior: "smooth",
        });
      });
    }
  });

  async function handleSend() {
    const text = inputText.trim();
    if (!text || sending) return;

    sending = true;
    sendError = null;
    try {
      const message = await sendMessage(chatId, text, password);
      appendMessage(message);
      inputText = "";
    } catch (err) {
      sendError = String(err);
    } finally {
      sending = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && !event.shiftKey) {
      event.preventDefault();
      handleSend();
    }
  }
</script>

<div class="chat-view">
  <div class="chat-header">
    <div class="chat-header-info">
      <h3 class="chat-title">{chatDetail?.chat_name ?? "Chat"}</h3>
      <span class="chat-members mono text-secondary">
        {chatDetail?.members.filter(m => m.is_online).length ?? 0}/{chatDetail?.members.length ?? 0} members
      </span>
    </div>
    <div class="header-actions">
      <button class="icon-btn" title="Invite" onclick={onGenerateInvite}>+</button>
      <button class="icon-btn" title="Inspector" onclick={onToggleInspector}>&#9776;</button>
    </div>
  </div>

  <div class="message-list" bind:this={messageListElement}>
    {#if loading}
      <div class="loading-state">
        <p class="text-muted">Loading messages...</p>
      </div>
    {:else if sortedMessages.length === 0}
      <div class="empty-state">
        <p class="text-muted">No messages yet</p>
        <p class="text-muted text-small">Send the first message to begin</p>
      </div>
    {:else}
      {#each sortedMessages as message (message.message_id)}
        <MessageRow
          {message}
          isOwnMessage={message.author_peer_id === identity?.peer_id}
          onSelect={onSelectMessage}
        />
      {/each}
    {/if}
  </div>

  {#if sendError}
    <div class="send-error text-error text-small">{sendError}</div>
  {/if}

  <div class="message-input-bar">
    <textarea
      class="message-input"
      placeholder="Type a message..."
      bind:value={inputText}
      onkeydown={handleKeydown}
      disabled={sending}
      rows={1}
    ></textarea>
    <button
      class="send-btn primary"
      onclick={handleSend}
      disabled={!inputText.trim() || sending}
    >
      {sending ? "..." : "Send"}
    </button>
  </div>
</div>

<style>
  .chat-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    flex: 1;
    min-width: 0;
  }

  .chat-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 16px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-secondary);
    height: var(--header-height);
    flex-shrink: 0;
  }

  .chat-header-info {
    display: flex;
    align-items: baseline;
    gap: 12px;
  }

  .chat-title {
    font-size: 15px;
    font-weight: 600;
  }

  .chat-members {
    font-size: 12px;
  }

  .icon-btn {
    background: none;
    border: none;
    padding: 4px 8px;
    font-size: 18px;
    color: var(--text-secondary);
    cursor: pointer;
  }

  .icon-btn:hover {
    color: var(--text-primary);
    background: var(--bg-hover);
    border-radius: var(--radius);
  }

  .header-actions {
    display: flex;
    gap: 4px;
    align-items: center;
  }

  .message-list {
    flex: 1;
    overflow-y: auto;
    padding: 12px 16px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .loading-state, .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    gap: 4px;
  }

  .send-error {
    padding: 4px 16px;
  }

  .message-input-bar {
    display: flex;
    gap: 8px;
    padding: 8px 16px;
    border-top: 1px solid var(--border);
    background: var(--bg-secondary);
    flex-shrink: 0;
  }

  .message-input {
    flex: 1;
    resize: none;
    min-height: 36px;
    max-height: 120px;
  }

  .send-btn {
    align-self: flex-end;
    min-width: 64px;
  }
</style>

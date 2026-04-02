<script lang="ts">
  import type { MessageInfo, DeliveryStatus } from "../lib/api";

  let {
    message,
    isOwnMessage,
    onSelect,
  }: {
    message: MessageInfo;
    isOwnMessage: boolean;
    onSelect?: (messageId: string) => void;
  } = $props();

  let formattedTime = $derived(formatWallClock(message.created_at));
  let deliveryLabel = $derived(formatDeliveryStatus(message.delivery_status));

  function formatWallClock(timestampSeconds: number): string {
    const date = new Date(timestampSeconds * 1000);
    const hours = String(date.getHours()).padStart(2, "0");
    const minutes = String(date.getMinutes()).padStart(2, "0");
    return `${hours}:${minutes}`;
  }

  function formatDeliveryStatus(status: DeliveryStatus): string {
    if (status === "queued") return "[queued]";
    if (status === "all") return "[ALL]";
    if (typeof status === "object" && "partial" in status) {
      return `[${status.partial.delivered}/${status.partial.total}]`;
    }
    return "";
  }

  function shortFingerprint(peerId: string): string {
    return peerId.slice(0, 8);
  }

  function handleClick() {
    onSelect?.(message.message_id);
  }
</script>

<button class="message-row" class:own={isOwnMessage} onclick={handleClick}>
  <div class="message-header">
    <span class="author-name">{message.author_name}</span>
    <span class="author-id mono">{shortFingerprint(message.author_peer_id)}</span>
    <span class="spacer"></span>
    <span class="lamport mono">L{message.lamport_ts}</span>
    <span class="time mono">{formattedTime}</span>
  </div>
  <div class="message-body selectable">{message.text}</div>
  {#if isOwnMessage}
    <div class="message-footer">
      <span
        class="delivery mono"
        class:queued={message.delivery_status === "queued"}
        class:delivered={message.delivery_status === "all"}
        class:partial={typeof message.delivery_status === "object" && "partial" in message.delivery_status}
      >
        {deliveryLabel}
      </span>
    </div>
  {/if}
</button>

<style>
  .message-row {
    display: flex;
    flex-direction: column;
    padding: 8px 12px;
    border: none;
    border-radius: var(--radius);
    background: var(--bg-message-other);
    text-align: left;
    width: 100%;
    cursor: pointer;
    transition: background 0.1s;
  }

  .message-row:hover {
    background: var(--bg-hover);
  }

  .message-row.own {
    background: var(--bg-message-own);
  }

  .message-row.own:hover {
    background: var(--bg-active);
  }

  .message-header {
    display: flex;
    align-items: baseline;
    gap: 6px;
    margin-bottom: 4px;
  }

  .author-name {
    font-weight: 600;
    font-size: 13px;
    color: var(--accent);
  }

  .own .author-name {
    color: var(--success);
  }

  .author-id {
    font-size: 11px;
    color: var(--text-muted);
  }

  .spacer {
    flex: 1;
  }

  .lamport {
    font-size: 11px;
    color: var(--text-muted);
  }

  .time {
    font-size: 11px;
    color: var(--text-secondary);
  }

  .message-body {
    font-size: 14px;
    line-height: 1.4;
    color: var(--text-primary);
    word-break: break-word;
    white-space: pre-wrap;
  }

  .message-footer {
    margin-top: 4px;
    display: flex;
    justify-content: flex-end;
  }

  .delivery {
    font-size: 11px;
  }

  .queued {
    color: var(--text-muted);
  }

  .partial {
    color: var(--warning);
  }

  .delivered {
    color: var(--success);
  }
</style>

<script lang="ts">
  import QRCode from "qrcode";
  import { generateInvite, joinChat } from "../lib/api";
  import { loadChats } from "../lib/stores/chats.svelte";

  let {
    mode,
    chatId,
    onClose,
  }: {
    mode: "generate" | "join";
    chatId?: string;
    onClose: () => void;
  } = $props();

  let inviteCode = $state("");
  let joinCode = $state("");
  let loading = $state(false);
  let error = $state<string | null>(null);
  let copied = $state(false);
  let qrDataUrl = $state<string | null>(null);

  $effect(() => {
    if (inviteCode) {
      QRCode.toDataURL(inviteCode, { width: 200, margin: 1 })
        .then((url: string) => {
          qrDataUrl = url;
        })
        .catch(() => {
          qrDataUrl = null;
        });
    } else {
      qrDataUrl = null;
    }
  });

  $effect(() => {
    if (mode === "generate" && chatId) {
      generateCode(chatId);
    }
  });

  async function generateCode(targetChatId: string) {
    loading = true;
    error = null;
    try {
      const result = await generateInvite(targetChatId);
      inviteCode = result.code;
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
    }
  }

  async function handleJoin() {
    const code = joinCode.trim();
    if (!code) return;

    loading = true;
    error = null;
    try {
      await joinChat(code);
      await loadChats();
      onClose();
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
    }
  }

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(inviteCode);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch {
      // clipboard API may not be available
    }
  }
</script>

<div class="dialog-overlay" role="presentation" onclick={onClose}>
  <div class="dialog" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => { if (e.key === "Escape") onClose(); }}>
    <div class="dialog-header">
      <h3>{mode === "generate" ? "Invite Code" : "Join Chat"}</h3>
      <button class="close-btn" onclick={onClose}>&times;</button>
    </div>

    <div class="dialog-body">
      {#if mode === "generate"}
        {#if loading}
          <p class="text-muted">Generating invite...</p>
        {:else if inviteCode}
          <div class="invite-display">
            {#if qrDataUrl}
              <img src={qrDataUrl} alt="Invite QR" class="qr-code" />
            {/if}
            <textarea
              class="invite-code mono selectable"
              readonly
              value={inviteCode}
              rows={4}
            ></textarea>
            <button class="primary" onclick={handleCopy}>
              {copied ? "Copied!" : "Copy"}
            </button>
          </div>
          <p class="text-muted text-small">
            Share this code with the person you want to invite.
          </p>
        {/if}
      {:else}
        <div class="join-form">
          <label class="field-label" for="join-code-input">Paste invite code</label>
          <textarea
            id="join-code-input"
            class="mono"
            bind:value={joinCode}
            placeholder="ghm://..."
            rows={4}
          ></textarea>
          <button
            class="primary"
            onclick={handleJoin}
            disabled={!joinCode.trim() || loading}
          >
            {loading ? "Joining..." : "Join Chat"}
          </button>
        </div>
      {/if}

      {#if error}
        <p class="text-error text-small">{error}</p>
      {/if}
    </div>
  </div>
</div>

<style>
  .dialog-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.6);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .dialog {
    background: var(--bg-primary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    width: 480px;
    max-width: 90vw;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
  }

  .dialog-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }

  .dialog-header h3 {
    font-size: 15px;
    font-weight: 600;
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

  .dialog-body {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .invite-display {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .invite-code {
    font-size: 11px;
    word-break: break-all;
    resize: none;
  }

  .join-form {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .field-label {
    font-size: 13px;
    color: var(--text-secondary);
  }

  .qr-code {
    display: block;
    margin: 0 auto 1rem;
    border-radius: 8px;
  }
</style>

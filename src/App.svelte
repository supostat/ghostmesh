<script lang="ts">
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { createChat, validatePassword } from "./lib/api";
  import {
    onMessageNew,
    onPeerConnected,
    onPeerDisconnected,
    onDeliveryAck,
    onNetworkStatus,
    onSyncComplete,
    onChatMemberJoined,
    onChatMemberLeft,
  } from "./lib/events";
  import {
    loadIdentity,
    getIdentityState,
    isIdentityLoading,
  } from "./lib/stores/identity.svelte";
  import {
    loadChats,
    selectChat,
    getSelectedChatId,
    loadChats as refreshChats,
  } from "./lib/stores/chats.svelte";
  import {
    loadMessages,
    appendMessage,
    updateDeliveryStatus,
    clearMessages,
  } from "./lib/stores/messages.svelte";
  import {
    setNetworkStatus,
    addPeerConnection,
    removePeerConnection,
    loadOutbox,
  } from "./lib/stores/network.svelte";

  import OnboardingScreen from "./components/OnboardingScreen.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import ChatView from "./components/ChatView.svelte";
  import InspectorPanel from "./components/InspectorPanel.svelte";
  import NetworkDashboard from "./components/NetworkDashboard.svelte";
  import Settings from "./components/Settings.svelte";
  import InviteDialog from "./components/InviteDialog.svelte";

  type Screen = "chat" | "network" | "settings";

  let currentScreen = $state<Screen>("chat");
  let inspectorOpen = $state(false);
  let selectedMessageId = $state<string | null>(null);
  let sessionPassword = $state("");
  let passwordPromptOpen = $state(false);
  let pendingAction = $state<(() => void) | null>(null);
  let passwordError = $state<string | null>(null);
  let passwordValidating = $state(false);

  // Invite dialog
  let inviteDialogOpen = $state(false);
  let inviteDialogMode = $state<"generate" | "join">("join");

  // Create chat dialog
  let createChatDialogOpen = $state(false);
  let newChatName = $state("");
  let createChatError = $state<string | null>(null);
  let createChatLoading = $state(false);

  let identity = $derived(getIdentityState());
  let identityLoading = $derived(isIdentityLoading());
  let selectedChatId = $derived(getSelectedChatId());

  let eventUnlisteners: UnlistenFn[] = [];

  $effect(() => {
    loadIdentity();
    return () => {
      eventUnlisteners.forEach((unlisten) => unlisten());
    };
  });

  $effect(() => {
    if (identity) {
      loadChats();
      setupEventListeners();
    }
  });

  async function setupEventListeners() {
    // Clean up previous listeners
    eventUnlisteners.forEach((unlisten) => unlisten());
    eventUnlisteners = [];

    eventUnlisteners.push(
      await onMessageNew((message) => {
        appendMessage(message);
        refreshChats();
      }),
      await onPeerConnected((event) => {
        addPeerConnection(event.peer_id, event.display_name);
      }),
      await onPeerDisconnected((event) => {
        removePeerConnection(event.peer_id);
      }),
      await onDeliveryAck((ack) => {
        updateDeliveryStatus(ack.message_id, "all");
        loadOutbox();
      }),
      await onNetworkStatus((status) => {
        setNetworkStatus(status);
      }),
      await onSyncComplete(() => {
        if (selectedChatId && sessionPassword) {
          loadMessages(selectedChatId, sessionPassword);
        }
        refreshChats();
      }),
      await onChatMemberJoined(() => {
        refreshChats();
      }),
      await onChatMemberLeft(() => {
        refreshChats();
      }),
    );
  }

  function withPassword(action: () => void) {
    if (sessionPassword) {
      action();
    } else {
      pendingAction = action;
      passwordError = null;
      passwordPromptOpen = true;
    }
  }

  async function handlePasswordSubmit() {
    if (!sessionPassword.trim()) return;
    passwordError = null;
    passwordValidating = true;
    try {
      const valid = await validatePassword(sessionPassword);
      if (!valid) {
        passwordError = "Wrong password — must match the password used during identity creation";
        return;
      }
      passwordPromptOpen = false;
      pendingAction?.();
      pendingAction = null;
    } catch (err) {
      passwordError = String(err);
    } finally {
      passwordValidating = false;
    }
  }

  function handleSelectChat(chatId: string) {
    withPassword(() => {
      selectChat(chatId);
      loadMessages(chatId, sessionPassword);
      currentScreen = "chat";
      selectedMessageId = null;
    });
  }

  function handleCreateChat() {
    newChatName = "";
    createChatError = null;
    createChatDialogOpen = true;
  }

  async function handleCreateChatSubmit() {
    const name = newChatName.trim();
    if (!name) return;

    withPassword(async () => {
      createChatLoading = true;
      createChatError = null;
      try {
        const chat = await createChat(name, sessionPassword);
        await loadChats();
        createChatDialogOpen = false;
        handleSelectChat(chat.chat_id);
      } catch (err) {
        createChatError = String(err);
      } finally {
        createChatLoading = false;
      }
    });
  }

  function handleJoinChat() {
    inviteDialogMode = "join";
    inviteDialogOpen = true;
  }

  function handleGenerateInvite() {
    if (!selectedChatId) return;
    inviteDialogMode = "generate";
    inviteDialogOpen = true;
  }

  function handleSelectMessage(messageId: string) {
    selectedMessageId = messageId;
    inspectorOpen = true;
  }

  function handleToggleInspector() {
    inspectorOpen = !inspectorOpen;
  }

  function handleOpenNetwork() {
    currentScreen = "network";
    clearMessages();
  }

  function handleOpenSettings() {
    currentScreen = "settings";
  }

  function handleCloseScreen() {
    currentScreen = "chat";
  }
</script>

{#if identityLoading}
  <main class="loading-screen">
    <h1 class="mono">GhostMesh</h1>
    <p class="text-muted">Loading...</p>
  </main>
{:else if !identity}
  <OnboardingScreen />
{:else}
  <main class="app-layout">
    <Sidebar
      onSelectChat={handleSelectChat}
      onCreateChat={handleCreateChat}
      onJoinChat={handleJoinChat}
      onOpenSettings={handleOpenSettings}
      onOpenNetwork={handleOpenNetwork}
    />

    <div class="main-content">
      {#if currentScreen === "network"}
        <NetworkDashboard onClose={handleCloseScreen} />
      {:else if currentScreen === "settings"}
        <Settings onClose={handleCloseScreen} />
      {:else if selectedChatId}
        <ChatView
          chatId={selectedChatId}
          password={sessionPassword}
          onSelectMessage={handleSelectMessage}
          onToggleInspector={handleToggleInspector}
          onGenerateInvite={handleGenerateInvite}
        />
        {#if inspectorOpen}
          <InspectorPanel
            {selectedMessageId}
            password={sessionPassword}
            chatId={selectedChatId}
            onClose={() => (inspectorOpen = false)}
          />
        {/if}
      {:else}
        <div class="no-chat-selected">
          <p class="text-muted">Select a chat or create a new one</p>
        </div>
      {/if}
    </div>
  </main>

  {#if passwordPromptOpen}
    <div class="dialog-overlay" role="presentation" onclick={() => { passwordPromptOpen = false; pendingAction = null; }}>
      <div class="dialog" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => { if (e.key === "Escape") { passwordPromptOpen = false; pendingAction = null; } }}>
        <div class="dialog-header">
          <h3>Enter Password</h3>
        </div>
        <div class="dialog-body">
          <p class="text-secondary text-small">
            Password is needed to decrypt messages and sign actions.
            It will be stored for this session.
          </p>
          <input
            type="password"
            bind:value={sessionPassword}
            placeholder="Your password"
            onkeydown={(e) => { if (e.key === "Enter") handlePasswordSubmit(); }}
            disabled={passwordValidating}
          />
          {#if passwordError}
            <p class="text-error text-small">{passwordError}</p>
          {/if}
          <button class="primary" onclick={handlePasswordSubmit} disabled={!sessionPassword.trim() || passwordValidating}>
            {passwordValidating ? "Verifying..." : "Unlock"}
          </button>
        </div>
      </div>
    </div>
  {/if}

  {#if createChatDialogOpen}
    <div class="dialog-overlay" role="presentation" onclick={() => (createChatDialogOpen = false)}>
      <div class="dialog" role="dialog" tabindex="-1" onclick={(e) => e.stopPropagation()} onkeydown={(e) => { if (e.key === "Escape") createChatDialogOpen = false; }}>
        <div class="dialog-header">
          <h3>Create Chat</h3>
          <button class="close-btn" onclick={() => (createChatDialogOpen = false)}>&times;</button>
        </div>
        <div class="dialog-body">
          <div class="field">
            <label class="field-label" for="new-chat-name">Chat Name</label>
            <input
              id="new-chat-name"
              type="text"
              bind:value={newChatName}
              placeholder="Enter chat name"
              onkeydown={(e) => { if (e.key === "Enter") handleCreateChatSubmit(); }}
            />
          </div>
          {#if createChatError}
            <p class="text-error text-small">{createChatError}</p>
          {/if}
          <button
            class="primary"
            onclick={handleCreateChatSubmit}
            disabled={!newChatName.trim() || createChatLoading}
          >
            {createChatLoading ? "Creating..." : "Create"}
          </button>
        </div>
      </div>
    </div>
  {/if}

  {#if inviteDialogOpen}
    <InviteDialog
      mode={inviteDialogMode}
      chatId={selectedChatId ?? undefined}
      onClose={() => (inviteDialogOpen = false)}
    />
  {/if}
{/if}

<style>
  .loading-screen {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100vh;
    gap: 8px;
  }

  .loading-screen h1 {
    font-size: 28px;
    color: var(--accent);
  }

  .app-layout {
    display: flex;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }

  .main-content {
    display: flex;
    flex: 1;
    min-width: 0;
    height: 100%;
  }

  .no-chat-selected {
    display: flex;
    align-items: center;
    justify-content: center;
    flex: 1;
  }

  /* Dialog styles (shared) */

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
    width: 400px;
    max-width: 90vw;
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

  .dialog-body {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 12px;
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

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .field-label {
    font-size: 13px;
    color: var(--text-secondary);
  }
</style>

<script lang="ts">
  import {
    getSettings,
    updateSettings,
    addManualPeer,
    exportIdentity,
    importIdentity,
    checkForUpdate,
    installUpdate,
    type Settings,
    type UpdateInfo,
  } from "../lib/api";
  import { getVersion } from "@tauri-apps/api/app";
  import { getIdentityState } from "../lib/stores/identity.svelte";

  let { onClose }: { onClose: () => void } = $props();

  let settings = $state<Settings | null>(null);
  let loading = $state(true);
  let saveError = $state<string | null>(null);
  let saveSuccess = $state(false);

  // Manual peer
  let manualPeerId = $state("");
  let manualAddress = $state("");
  let peerAddError = $state<string | null>(null);
  let peerAddSuccess = $state(false);

  // Updates
  let appVersion = $state("");
  let updateAvailable = $state<UpdateInfo | null>(null);
  let updateChecking = $state(false);
  let updateInstalling = $state(false);
  let updateError = $state<string | null>(null);

  // Identity export/import
  let exportPassword = $state("");
  let exportData = $state("");
  let exportError = $state<string | null>(null);
  let importPassword = $state("");
  let importData = $state("");
  let importError = $state<string | null>(null);

  let identity = $derived(getIdentityState());

  $effect(() => {
    loadSettings();
    loadAppVersion();
  });

  async function loadAppVersion() {
    try {
      appVersion = await getVersion();
    } catch {
      appVersion = "unknown";
    }
  }

  async function handleCheckUpdate() {
    updateChecking = true;
    updateError = null;
    updateAvailable = null;
    try {
      updateAvailable = await checkForUpdate();
    } catch (err) {
      updateError = String(err);
    } finally {
      updateChecking = false;
    }
  }

  async function handleInstallUpdate() {
    updateInstalling = true;
    updateError = null;
    try {
      await installUpdate();
    } catch (err) {
      updateError = String(err);
      updateInstalling = false;
    }
  }

  async function loadSettings() {
    loading = true;
    try {
      settings = await getSettings();
    } catch (err) {
      saveError = String(err);
    } finally {
      loading = false;
    }
  }

  async function handleSave() {
    if (!settings) return;
    saveError = null;
    saveSuccess = false;
    try {
      settings = await updateSettings(settings);
      saveSuccess = true;
      setTimeout(() => (saveSuccess = false), 2000);
    } catch (err) {
      saveError = String(err);
    }
  }

  async function handleAddPeer() {
    const peerId = manualPeerId.trim();
    const address = manualAddress.trim();
    if (!peerId || !address) return;

    peerAddError = null;
    peerAddSuccess = false;
    try {
      await addManualPeer(peerId, address);
      manualPeerId = "";
      manualAddress = "";
      peerAddSuccess = true;
      setTimeout(() => (peerAddSuccess = false), 2000);
    } catch (err) {
      peerAddError = String(err);
    }
  }

  async function handleExport() {
    const password = exportPassword.trim();
    if (!password) return;

    exportError = null;
    try {
      const bytes = await exportIdentity(password);
      exportData = btoa(String.fromCharCode(...bytes));
    } catch (err) {
      exportError = String(err);
    }
  }

  async function handleImport() {
    const password = importPassword.trim();
    const data = importData.trim();
    if (!password || !data) return;

    importError = null;
    try {
      const binaryString = atob(data);
      const bytes = Array.from(binaryString, (char) => char.charCodeAt(0));
      await importIdentity(bytes, password);
      importData = "";
      importPassword = "";
    } catch (err) {
      importError = String(err);
    }
  }
</script>

<div class="settings-view">
  <div class="settings-header">
    <h2>Settings</h2>
    <button class="close-btn" onclick={onClose}>&times;</button>
  </div>

  <div class="settings-content">
    {#if loading}
      <p class="text-muted">Loading settings...</p>
    {:else if settings}
      <section class="section">
        <h3 class="section-title">Identity</h3>
        {#if identity}
          <div class="identity-info">
            <div class="field-row">
              <span class="field-label">Peer ID</span>
              <span class="mono selectable text-small">{identity.peer_id}</span>
            </div>
            <div class="field-row">
              <span class="field-label">Fingerprint</span>
              <span class="mono selectable text-small">{identity.fingerprint}</span>
            </div>
          </div>
        {/if}
      </section>

      <section class="section">
        <h3 class="section-title">General</h3>
        <div class="field">
          <label class="field-label" for="settings-display-name">Display Name</label>
          <input
            id="settings-display-name"
            type="text"
            bind:value={settings.display_name}
          />
        </div>
      </section>

      <section class="section">
        <h3 class="section-title">Network</h3>
        <div class="field">
          <label class="field-label" for="settings-port">Listen Port</label>
          <input
            id="settings-port"
            type="number"
            bind:value={settings.listen_port}
            min={1024}
            max={65535}
          />
        </div>
        <div class="field-inline">
          <input
            id="settings-mdns"
            type="checkbox"
            bind:checked={settings.mdns_enabled}
          />
          <label for="settings-mdns">Enable mDNS Discovery</label>
        </div>
      </section>

      <section class="section">
        <h3 class="section-title">Updates</h3>
        <div class="field-row">
          <span class="field-label">Current Version</span>
          <span class="mono text-small">{appVersion}</span>
        </div>
        <div class="field-inline">
          <input
            id="settings-auto-update"
            type="checkbox"
            bind:checked={settings.auto_update_enabled}
          />
          <label for="settings-auto-update">Check for updates on startup</label>
        </div>
        <div class="update-actions">
          <button
            onclick={handleCheckUpdate}
            disabled={updateChecking || updateInstalling}
          >
            {updateChecking ? "Checking..." : "Check for Updates"}
          </button>
          {#if updateAvailable}
            <div class="update-info">
              <span class="text-success text-small">
                Version {updateAvailable.version} available
              </span>
              {#if updateAvailable.body}
                <p class="text-small text-muted update-notes">{updateAvailable.body}</p>
              {/if}
              <button
                class="primary"
                onclick={handleInstallUpdate}
                disabled={updateInstalling}
              >
                {updateInstalling ? "Installing..." : "Install and Restart"}
              </button>
            </div>
          {/if}
          {#if updateAvailable === null && !updateChecking && !updateError}
            <!-- initial state or no update found after check -->
          {/if}
          {#if updateError}
            <span class="text-error text-small">{updateError}</span>
          {/if}
        </div>
      </section>

      <div class="save-bar">
        <button class="primary" onclick={handleSave}>Save Settings</button>
        {#if saveSuccess}
          <span class="text-success text-small">Saved</span>
        {/if}
        {#if saveError}
          <span class="text-error text-small">{saveError}</span>
        {/if}
      </div>

      <section class="section">
        <h3 class="section-title">Add Manual Peer</h3>
        <div class="field">
          <label class="field-label" for="manual-peer-id">Peer ID (hex)</label>
          <input
            id="manual-peer-id"
            type="text"
            class="mono"
            bind:value={manualPeerId}
            placeholder="0a1b2c3d..."
          />
        </div>
        <div class="field">
          <label class="field-label" for="manual-address">Address (host:port)</label>
          <input
            id="manual-address"
            type="text"
            class="mono"
            bind:value={manualAddress}
            placeholder="192.168.1.10:9473"
          />
        </div>
        <button onclick={handleAddPeer} disabled={!manualPeerId.trim() || !manualAddress.trim()}>
          Add Peer
        </button>
        {#if peerAddSuccess}
          <span class="text-success text-small">Peer added</span>
        {/if}
        {#if peerAddError}
          <span class="text-error text-small">{peerAddError}</span>
        {/if}
      </section>

      <section class="section">
        <h3 class="section-title">Export Identity</h3>
        <div class="field">
          <label class="field-label" for="export-password">Password</label>
          <input
            id="export-password"
            type="password"
            bind:value={exportPassword}
          />
        </div>
        <button onclick={handleExport} disabled={!exportPassword.trim()}>Export</button>
        {#if exportData}
          <textarea class="mono text-small selectable" readonly rows={3} value={exportData}></textarea>
        {/if}
        {#if exportError}
          <span class="text-error text-small">{exportError}</span>
        {/if}
      </section>

      <section class="section">
        <h3 class="section-title">Import Identity</h3>
        <div class="field">
          <label class="field-label" for="import-data">Encrypted Data (base64)</label>
          <textarea
            id="import-data"
            class="mono"
            bind:value={importData}
            rows={3}
            placeholder="Paste base64 data..."
          ></textarea>
        </div>
        <div class="field">
          <label class="field-label" for="import-password">Password</label>
          <input
            id="import-password"
            type="password"
            bind:value={importPassword}
          />
        </div>
        <button class="danger" onclick={handleImport} disabled={!importData.trim() || !importPassword.trim()}>
          Import (overwrites current identity)
        </button>
        {#if importError}
          <span class="text-error text-small">{importError}</span>
        {/if}
      </section>
    {/if}
  </div>
</div>

<style>
  .settings-view {
    display: flex;
    flex-direction: column;
    height: 100%;
    flex: 1;
    min-width: 0;
  }

  .settings-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 16px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-secondary);
    height: var(--header-height);
    flex-shrink: 0;
  }

  .settings-header h2 {
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

  .settings-content {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 20px;
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
    padding-bottom: 4px;
    border-bottom: 1px solid var(--border);
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

  .field-inline {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .field-inline input[type="checkbox"] {
    width: 16px;
    height: 16px;
  }

  .field-row {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .identity-info {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 8px;
    background: var(--bg-secondary);
    border-radius: var(--radius);
  }

  .save-bar {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .update-actions {
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-start;
  }

  .update-info {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px;
    background: var(--bg-secondary);
    border-radius: var(--radius);
    width: 100%;
  }

  .update-notes {
    white-space: pre-wrap;
    max-height: 80px;
    overflow-y: auto;
  }
</style>

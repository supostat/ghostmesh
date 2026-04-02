<script lang="ts">
  import { createIdentity } from "../lib/api";
  import { setIdentity } from "../lib/stores/identity.svelte";

  let { onCreated }: { onCreated: (password: string) => void } = $props();

  let displayName = $state("");
  let password = $state("");
  let passwordConfirm = $state("");
  let loading = $state(false);
  let error = $state<string | null>(null);

  let passwordsMatch = $derived(password === passwordConfirm);
  let canSubmit = $derived(
    displayName.trim().length > 0 &&
      password.length >= 8 &&
      passwordsMatch &&
      !loading,
  );

  async function handleCreate() {
    if (!canSubmit) return;

    loading = true;
    error = null;
    try {
      const identity = await createIdentity(displayName.trim(), password);
      setIdentity(identity);
      onCreated(password);
    } catch (err) {
      error = String(err);
    } finally {
      loading = false;
    }
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === "Enter" && canSubmit) {
      handleCreate();
    }
  }
</script>

<div class="onboarding">
  <div class="onboarding-card">
    <div class="logo-area">
      <h1 class="mono">GhostMesh</h1>
      <p class="text-secondary">P2P Messenger with BBS Philosophy</p>
    </div>

    <div class="form-area">
      <h2>Create Identity</h2>
      <p class="text-secondary text-small">
        Your identity is stored locally. The password encrypts your private keys.
      </p>

      <div class="field">
        <label class="field-label" for="onboard-name">Display Name</label>
        <input
          id="onboard-name"
          type="text"
          bind:value={displayName}
          onkeydown={handleKeydown}
          placeholder="Your name"
          autocomplete="off"
        />
      </div>

      <div class="field">
        <label class="field-label" for="onboard-password">Password (min 8 characters)</label>
        <input
          id="onboard-password"
          type="password"
          bind:value={password}
          onkeydown={handleKeydown}
          placeholder="Password for key encryption"
        />
      </div>

      <div class="field">
        <label class="field-label" for="onboard-confirm">Confirm Password</label>
        <input
          id="onboard-confirm"
          type="password"
          bind:value={passwordConfirm}
          onkeydown={handleKeydown}
          placeholder="Repeat password"
        />
        {#if passwordConfirm && !passwordsMatch}
          <span class="text-error text-small">Passwords do not match</span>
        {/if}
      </div>

      {#if error}
        <p class="text-error text-small">{error}</p>
      {/if}

      <button
        class="primary create-btn"
        onclick={handleCreate}
        disabled={!canSubmit}
      >
        {loading ? "Creating..." : "Create Identity"}
      </button>
    </div>
  </div>
</div>

<style>
  .onboarding {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100vh;
    width: 100vw;
    background: var(--bg-primary);
  }

  .onboarding-card {
    display: flex;
    flex-direction: column;
    gap: 24px;
    padding: 32px;
    background: var(--bg-secondary);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    width: 400px;
    max-width: 90vw;
  }

  .logo-area {
    text-align: center;
  }

  .logo-area h1 {
    font-size: 28px;
    color: var(--accent);
    margin-bottom: 4px;
  }

  .form-area {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .form-area h2 {
    font-size: 16px;
    font-weight: 600;
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

  .create-btn {
    margin-top: 8px;
    padding: 10px 16px;
    font-size: 14px;
  }
</style>

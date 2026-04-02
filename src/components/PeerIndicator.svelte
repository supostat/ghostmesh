<script lang="ts">
  let { connected, count }: { connected: boolean; count?: number } = $props();
</script>

<span class="peer-indicator" class:connected class:disconnected={!connected}>
  <span class="dot"></span>
  {#if count !== undefined}
    <span class="count mono">{count}</span>
  {/if}
</span>

<style>
  .peer-indicator {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .connected .dot {
    background: var(--success);
    box-shadow: 0 0 4px var(--success);
  }

  .disconnected .dot {
    background: var(--text-muted);
  }

  .count {
    font-size: 12px;
    color: var(--text-secondary);
  }
</style>

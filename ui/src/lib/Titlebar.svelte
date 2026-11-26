<script lang="ts">
  import type { Snippet } from 'svelte'
  import { inTauri } from './ipc'
  import { platform, remote } from './state.svelte'

  let { lead, center, trail }: { lead?: Snippet; center?: Snippet; trail?: Snippet } = $props()
  const traffic = inTauri && platform.mac
</script>

<header class="titlebar" class:traffic data-tauri-drag-region>
  <div class="lead" data-tauri-drag-region>{#if remote.host}<span class="remote" title="Connected to {remote.host}">{remote.host}</span>{/if}{@render lead?.()}</div>
  <div class="center" data-tauri-drag-region>{@render center?.()}</div>
  <div class="trail" data-tauri-drag-region>{@render trail?.()}</div>
</header>

<style>
  .titlebar { flex: none; display: grid; grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr); align-items: center; gap: 12px; height: var(--titlebar-h); padding: 0 12px; background: var(--chrome); box-sizing: border-box; -webkit-user-select: none; user-select: none; }
  .traffic .lead { padding-left: calc(var(--traffic-pad) - 12px); }
  .lead, .trail { display: flex; align-items: center; gap: 4px; min-width: 0; align-self: stretch; }
  .trail { justify-content: flex-end; }
  .trail :global(.icon-button) { width: 32px; height: 32px; color: var(--fg); background: var(--glass); box-shadow: var(--glass-rim); }
  .trail :global(.icon-button:hover:not(:disabled)) { background: var(--glass-strong); }
  .remote { flex: none; padding: 2px 10px; border-radius: 999px; background: var(--button-bg); color: var(--button-fg); font-size: 0.846rem; font-weight: 500; box-shadow: inset 0 0.5px 0 #ffffff47; }
  .center { display: flex; align-items: center; justify-content: center; max-width: 44vw; min-width: 0; align-self: stretch; }
</style>

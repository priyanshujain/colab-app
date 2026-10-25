<script lang="ts">
  import type { Snippet } from 'svelte'
  import { inTauri } from './ipc'

  let { lead, center, trail }: { lead?: Snippet; center?: Snippet; trail?: Snippet } = $props()
  const traffic = inTauri && /Mac/.test(navigator.userAgent)
</script>

<header class="titlebar" class:traffic data-tauri-drag-region>
  <div class="lead" data-tauri-drag-region>{@render lead?.()}</div>
  <div class="center" data-tauri-drag-region>{@render center?.()}</div>
  <div class="trail" data-tauri-drag-region>{@render trail?.()}</div>
</header>

<style>
  .titlebar { flex: none; display: grid; grid-template-columns: minmax(0, 1fr) auto minmax(0, 1fr); align-items: center; gap: 12px; height: var(--titlebar-h); padding: 0 12px; background: var(--panel); border-bottom: 1px solid var(--border); box-sizing: border-box; -webkit-user-select: none; user-select: none; }
  .traffic .lead { padding-left: calc(var(--traffic-pad) - 12px); }
  .lead, .trail { display: flex; align-items: center; gap: 4px; min-width: 0; align-self: stretch; }
  .trail { justify-content: flex-end; }
  .center { display: flex; align-items: center; justify-content: center; max-width: 44vw; min-width: 0; align-self: stretch; }
</style>

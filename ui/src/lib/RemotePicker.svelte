<script lang="ts">
  import { onMount } from 'svelte'
  import type { Api, Listing } from './ipc'

  let { api, host, start, onclose, onopen }: { api: Api; host: string; start: string; onclose: () => void; onopen: (path: string) => void } = $props()
  let listing = $state<Listing>({ path: '', entries: [] })
  let value = $state('')
  let selected = $state(0)
  let error = $state('')

  const dir = $derived(value.slice(0, value.lastIndexOf('/') + 1))
  const typed = $derived(value.slice(value.lastIndexOf('/') + 1).toLowerCase())
  const items = $derived([...(listing.path === '/' ? [] : [{ name: '..', dir: true }]), ...listing.entries].filter((e) => e.name.toLowerCase().startsWith(typed)))
  const join = (name: string) => (name === '..' ? listing.path.replace(/\/[^/]+$/, '') || '/' : `${listing.path === '/' ? '' : listing.path}/${name}`)

  async function go(path: string) {
    try {
      listing = await api.listDir(path)
      value = listing.path === '/' ? '/' : `${listing.path}/`
      selected = 0
      error = ''
    } catch (e) {
      error = String(e)
    }
  }

  function choose(entry: { name: string; dir: boolean }) {
    if (entry.dir) go(join(entry.name))
    else onopen(join(entry.name))
  }

  function accept() {
    if (value.endsWith('.ipynb')) onopen(value)
    else if (items[selected]) choose(items[selected])
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Escape') onclose()
    else if (e.key === 'ArrowDown') (e.preventDefault(), (selected = Math.min(selected + 1, items.length - 1)))
    else if (e.key === 'ArrowUp') (e.preventDefault(), (selected = Math.max(selected - 1, 0)))
    else if (e.key === 'Enter') (e.preventDefault(), accept())
  }

  $effect(() => {
    if (dir && listing.path && dir.replace(/(.)\/$/, '$1') !== listing.path) go(dir)
  })

  onMount(() => go(start))
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={(e) => e.target === e.currentTarget && onclose()}>
  <div class="picker" role="dialog" aria-label="Open File">
    <div class="head"><span>Open File</span><span class="host">{host}</span></div>
    <!-- svelte-ignore a11y_autofocus -->
    <input bind:value onkeydown={key} oninput={() => (selected = 0)} spellcheck="false" autofocus aria-label="Path" />
    {#if error}<p class="error">{error}</p>{/if}
    <ul>
      {#each items as entry, i (entry.name)}
        <li><button class:selected={i === selected} onclick={() => choose(entry)} onmouseenter={() => (selected = i)}>{entry.name}{entry.dir ? '/' : ''}</button></li>
      {/each}
    </ul>
    <div class="actions">
      <button onclick={onclose}>Cancel</button>
      <button class="primary" onclick={accept}>OK</button>
    </div>
  </div>
</div>

<style>
  .backdrop { position: fixed; inset: 0; display: grid; place-items: start center; padding-top: calc(var(--titlebar-h) + 4px); z-index: 30; }
  .picker { width: min(600px, 90vw); background: var(--popover); border: 1px solid var(--border-strong); border-radius: 8px; padding: 6px; box-shadow: 0 12px 40px #000a; display: flex; flex-direction: column; gap: 4px; }
  .head { display: flex; justify-content: space-between; padding: 2px 6px; color: var(--muted); font-size: 0.923rem; }
  .host { color: var(--faint); }
  input { font-family: var(--mono); font-size: 0.923rem; }
  ul { list-style: none; margin: 0; padding: 0; max-height: 50vh; overflow-y: auto; }
  li button { width: 100%; justify-content: flex-start; border-color: transparent; font-family: var(--mono); font-size: 0.923rem; padding: 3px 8px; }
  li button.selected { background: var(--hover-strong); }
  .error { color: var(--error); margin: 2px 6px; font-size: 0.923rem; }
  .actions { display: flex; justify-content: flex-end; gap: 6px; padding: 4px 2px 2px; }
</style>

<script lang="ts">
  let { value = $bindable(), fonts, label }: { value: string; fonts: string[] | null; label: string } = $props()
  let open = $state(false)
  let query = $state('')
  let active = $state(0)
  let root: HTMLElement | undefined = $state()
  let list: HTMLElement | undefined = $state()
  const id = $props.id()
  const shown = $derived((fonts ?? []).filter((f) => f.toLowerCase().includes(query.toLowerCase())))

  function toggle() {
    open = !open
    query = ''
    active = Math.max(0, shown.indexOf(value))
  }

  function close() {
    open = false
    root?.querySelector('button')?.focus()
  }

  function pick(font: string) {
    value = font
    close()
  }

  function onkey(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') active = Math.min(active + 1, shown.length - 1)
    else if (e.key === 'ArrowUp') active = Math.max(active - 1, 0)
    else if (e.key === 'Enter' && shown[active]) pick(shown[active])
    else if (e.key === 'Escape') close()
    else return
    e.preventDefault()
    e.stopPropagation()
    list?.children[active]?.scrollIntoView({ block: 'nearest' })
  }
</script>

<svelte:window onclick={(e) => open && !root?.contains(e.target as Node) && (open = false)} />

<div class="picker" bind:this={root}>
  <button type="button" class="current outline" style:font-family="'{value}', var(--sans)" aria-label="{label}: {value}" aria-haspopup="listbox" aria-expanded={open} onclick={toggle}>
    <span class="text">{value}</span><svg class="caret" width="8" height="12" viewBox="0 0 8 12" aria-hidden="true"><path d="M1 4.5L4 1.5 7 4.5M1 7.5L4 10.5 7 7.5" fill="none" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" stroke-linejoin="round" /></svg>
  </button>
  {#if open}
    <div class="popup pop">
      <input {@attach (n) => n.focus()} placeholder="Search fonts" aria-label="Search fonts" role="combobox" aria-expanded="true" aria-controls="{id}-list" aria-activedescendant={shown[active] ? `${id}-${active}` : undefined} bind:value={query} oninput={() => (active = 0)} onkeydown={onkey} />
      <div class="list" id="{id}-list" role="listbox" aria-label={label} bind:this={list}>
        {#if fonts === null}
          <div class="empty">Loading fonts...</div>
        {:else}
          {#each shown as font, i (font)}
            <button type="button" tabindex="-1" role="option" id="{id}-{i}" aria-selected={font === value} class:active={i === active} style:font-family="'{font}', var(--sans)" onmouseenter={() => (active = i)} onclick={() => pick(font)}>
              <span class="check">{font === value ? '✓' : ''}</span>{font}
            </button>
          {:else}
            <div class="empty">No font matches</div>
          {/each}
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .picker { position: relative; flex: 0 1 200px; min-width: 96px; }
  .current { width: 100%; height: 28px; justify-content: space-between; gap: 8px; padding: 0 10px 0 12px; font-weight: 400; }
  .text { overflow: hidden; text-overflow: ellipsis; }
  .caret { flex: none; color: var(--muted); }
  .popup { position: absolute; z-index: 20; top: calc(100% + 6px); right: 0; width: max(100%, 232px); box-sizing: border-box; padding: 6px; overflow: hidden; }
  .popup input { width: 100%; box-sizing: border-box; margin-bottom: 4px; border-radius: 999px; padding: 4px 12px; }
  .list { max-height: 280px; overflow-y: auto; }
  .list button { display: flex; width: 100%; gap: 4px; padding: 4px 8px 4px 4px; border: 0; border-radius: 8px; font-size: 1.077rem; background: transparent; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .list button.active { background: var(--accent); color: var(--button-fg); }
  .check { width: 16px; flex: none; text-align: center; font-family: var(--sans); font-size: 0.846rem; }
  .empty { padding: 8px 10px; color: var(--muted); }
</style>

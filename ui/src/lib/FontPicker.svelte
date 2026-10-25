<script lang="ts">
  let { value = $bindable(), fonts }: { value: string; fonts: string[] | null } = $props()
  let open = $state(false)
  let query = $state('')
  let active = $state(0)
  let list: HTMLElement | undefined = $state()
  const shown = $derived((fonts ?? []).filter((f) => f.toLowerCase().includes(query.toLowerCase())))

  function toggle() {
    open = !open
    query = ''
    active = Math.max(0, shown.indexOf(value))
  }

  function pick(font: string) {
    value = font
    open = false
  }

  function onkey(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') active = Math.min(active + 1, shown.length - 1)
    else if (e.key === 'ArrowUp') active = Math.max(active - 1, 0)
    else if (e.key === 'Enter' && shown[active]) pick(shown[active])
    else if (e.key === 'Escape') open = false
    else return
    e.preventDefault()
    e.stopPropagation()
    list?.children[active]?.scrollIntoView({ block: 'nearest' })
  }
</script>

<svelte:window onclick={(e) => open && !(e.target as HTMLElement).closest('.picker') && (open = false)} />

<div class="picker">
  <button type="button" class="current" style:font-family="'{value}', var(--sans)" onclick={toggle}>{value}<span class="caret">▾</span></button>
  {#if open}
    <div class="popup">
      <input {@attach (n) => n.focus()} placeholder="Search fonts" bind:value={query} oninput={() => (active = 0)} onkeydown={onkey} />
      <div class="list" role="listbox" bind:this={list}>
        {#if fonts === null}
          <div class="empty">Loading fonts...</div>
        {:else}
          {#each shown as font, i (font)}
            <button type="button" role="option" aria-selected={font === value} class:active={i === active} class:selected={font === value}
              style:font-family="'{font}', var(--sans)" onmouseenter={() => (active = i)} onclick={() => pick(font)}>{font}</button>
          {:else}
            <div class="empty">No font matches</div>
          {/each}
        {/if}
      </div>
    </div>
  {/if}
</div>

<style>
  .picker { position: relative; width: min(280px, 55%); }
  .current { width: 100%; justify-content: space-between; background: var(--editor-bg); border: 1px solid var(--border); border-radius: 5px; padding: 5px 9px; font-size: 0.95rem; }
  .caret { color: var(--muted); font-family: var(--ui-font, inherit); font-size: 0.75rem; }
  .popup { position: absolute; z-index: 20; top: calc(100% + 4px); left: 0; right: 0; background: var(--panel, var(--editor-bg)); border: 1px solid var(--border); border-radius: 6px; box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35); overflow: hidden; }
  input { width: 100%; box-sizing: border-box; border: 0; border-bottom: 1px solid var(--border); border-radius: 0; padding: 7px 9px; background: transparent; outline: none; }
  .list { max-height: 300px; overflow-y: auto; padding: 4px; }
  .list button { display: block; width: 100%; text-align: left; padding: 6px 8px; border: 0; border-radius: 4px; font-size: 1rem; background: transparent; }
  .list button.active { background: var(--hover-strong); }
  .list button.selected { color: var(--accent); }
  .empty { padding: 8px; color: var(--muted); }
</style>

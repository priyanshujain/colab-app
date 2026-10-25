<script lang="ts">
  let { path, dirty, onrename, onmenu }: { path: string; dirty: boolean; onrename: (name: string) => Promise<void>; onmenu: () => void } = $props()

  const name = $derived(path.split('/').at(-1) ?? path)
  let draft = $state<string | null>(null)
  let error = $state('')

  function start(e: MouseEvent) {
    draft = name
    error = ''
    const field = (e.currentTarget as HTMLElement).parentElement!
    requestAnimationFrame(() => field.querySelector('input')?.setSelectionRange(0, name.replace(/\.ipynb$/, '').length))
  }

  async function commit() {
    if (draft === null) return
    const next = draft.trim()
    if (!next || next === name) return (draft = null)
    try {
      await onrename(next)
      draft = null
    } catch (e) {
      error = String(e)
    }
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'Enter') commit()
    else if (e.key === 'Escape') (e.stopPropagation(), (draft = null), (error = ''))
  }

  const focus = (node: HTMLInputElement) => node.focus()
</script>

<div class="file" data-tauri-drag-region>
  {#if draft === null}
    <button class="name" onclick={start} oncontextmenu={(e) => (e.preventDefault(), onmenu())} title={path}>{name}</button>
    {#if dirty}<span class="dot" title="Unsaved changes"></span>{/if}
  {:else}
    <input use:focus bind:value={draft} onkeydown={key} onblur={() => error || (draft = null)} class:invalid={error} aria-label="Notebook name" spellcheck="false" />
  {/if}
  {#if error}<span class="error" title={error}>{error}</span>{/if}
</div>

<style>
  .file { position: relative; display: flex; align-items: center; justify-content: center; gap: 6px; min-width: 0; height: 100%; }
  .name { height: 26px; padding: 0 9px; font-size: 1rem; font-weight: 500; color: var(--fg); min-width: 0; overflow: hidden; text-overflow: ellipsis; display: block; line-height: 24px; }
  .name:hover:not(:disabled) { border-color: transparent; background: var(--hover-strong); }
  .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--muted); flex: none; margin-left: -4px; }
  input { width: 260px; max-width: 100%; height: 26px; box-sizing: border-box; padding: 0 9px; font-size: 1rem; text-align: center; }
  .invalid { border-color: var(--error); }
  .error { position: absolute; top: calc(100% - 4px); left: 50%; transform: translateX(-50%); z-index: 30; max-width: 360px; padding: 4px 9px; border-radius: 5px; background: var(--popover); border: 1px solid color-mix(in srgb, var(--error) 37.6%, transparent); color: var(--error); font-size: 0.923rem; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
</style>

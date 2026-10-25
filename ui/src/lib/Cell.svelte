<script lang="ts">
  import type { Cell } from '../bindings/Cell'
  import type { InputRequest } from '../bindings/InputRequest'
  import Editor from './Editor.svelte'
  import Icon from './Icon.svelte'
  import { DOWN, PENCIL, TRASH, UP, MORE } from './icons'
  import { settings } from './state.svelte'
  import type { Match } from './find'
  import type { Lsp } from './lsp'
  import { renderMarkdown } from './markdown'
  import Outputs from './outputs/Outputs.svelte'
  import { duration, lastRun } from './timing'
  let moreOpen = $state(false)

  let { cell, path, metadata, selected, editing, running, lsp, diagnostics, marks, match, reveal, input, onselect, onedit, onchange, onkey, onrun, onreply, first, last, onmove, ontoggle, ondelete }: {
    cell: Cell
    path: string
    metadata: Record<string, unknown>
    selected: boolean
    editing: boolean
    running: boolean
    lsp: Lsp
    diagnostics: unknown[]
    marks: Match[]
    match: Match | null
    reveal: { line: number; character: number } | null
    input: InputRequest | null
    onselect: () => void
    onedit: () => void
    onchange: (s: string) => void
    onkey: (key: 'Shift-Enter' | 'Mod-Enter' | 'Escape') => void
    onrun: () => void
    onreply: (value: string) => void
    first: boolean
    last: boolean
    onmove: (delta: number) => void
    ontoggle: () => void
    ondelete: () => void
  } = $props()

  const code = $derived(cell.cell_type === 'code')
  let value = $state('')
  const run = $derived(code ? lastRun(cell) : null)
  let now = $state(Date.now())
  $effect(() => {
    if (!running) return
    now = Date.now()
    const t = setInterval(() => (now = Date.now()), 100)
    return () => clearInterval(t)
  })
</script>

<div class="cell" class:selected class:editing class:code data-cell-id={cell.id} role="presentation" onmousedown={(e) => (e.target as Element).closest('.cm-editor') || onselect()}>
  <div class="gutter">
    {#if code}
      <span class="count">[{running ? '*' : (cell.execution_count ?? ' ')}]</span>
      {#if running && run}
        <span class="took">{duration(now - run.start)}</span>
      {:else if run && run.ms !== null}
        <span class="took" class:ok={run.ok} class:failed={!run.ok} title={run.ok ? 'Succeeded' : 'Failed'}><svg viewBox="0 0 16 16" width="10" height="10"><path d={run.ok ? 'M3 8.5l3 3 7-7' : 'M4 4l8 8M12 4l-8 8'} fill="none" stroke="currentColor" stroke-width="2" /></svg>{duration(run.ms)}</span>
      {/if}
      <button class="run" class:running title="Run cell (Shift+Enter)" onmousedown={(e) => e.stopPropagation()} onclick={onrun} aria-label="Run cell">
        {#if running}<span class="spinner"></span>{:else}<svg viewBox="0 0 16 16" width="12" height="12"><path d="M4 2.5v11l9-5.5z" fill="currentColor" /></svg>{/if}
      </button>
    {/if}
  </div>
  <div class="body">
    <div class="tools" role="toolbar" aria-label="Cell actions">
      <button onclick={() => onmove(-1)} disabled={first} title="Move up" aria-label="Move cell up"><Icon d={UP} size={13} /></button>
      <button onclick={() => onmove(1)} disabled={last} title="Move down" aria-label="Move cell down"><Icon d={DOWN} size={13} /></button>
      {#if !code && !editing}<button onclick={onedit} title="Edit (Enter)" aria-label="Edit markdown"><Icon d={PENCIL} size={13} /></button>{/if}
      {#if !code && editing}<button onclick={() => onkey('Escape')} title="Close editor (Esc)" class="label">Close</button>{/if}
      <button onclick={ondelete} title="Delete cell (D D)" aria-label="Delete cell"><Icon d={TRASH} size={13} /></button>
      <div class="more">
        <button onclick={() => (moreOpen = !moreOpen)} title="More actions" aria-label="More cell actions" aria-expanded={moreOpen}><Icon d={MORE} size={13} /></button>
        {#if moreOpen}
          <div class="menu" role="menu">
            <button role="menuitem" onclick={() => ((moreOpen = false), ontoggle())}>Convert to {code ? 'Markdown' : 'Code'}<kbd>{code ? 'M' : 'Y'}</kbd></button>
          </div>
        {/if}
      </div>
    </div>
    {#if code || editing || match}
      <div class="source">
        <Editor source={cell.source} language={code ? 'python' : 'markdown'} focused={editing} lsp={code ? lsp : undefined} {diagnostics} {marks} {match} {reveal} {onchange} onfocus={onedit} {onkey} />
      </div>
    {:else}
      <div class="markdown rendered" role="presentation" ondblclick={() => settings.dblclickMarkdown && onedit()}>{@html renderMarkdown(cell.source || '*Empty markdown cell, press Enter or the pencil to edit*')}</div>
    {/if}
    {#if code}
      <Outputs {cell} {path} {metadata} />
    {/if}
    {#if input}
      <form class="stdin" onsubmit={(e) => (e.preventDefault(), onreply(value), (value = ''))}>
        <label for="stdin-{cell.id}">{input.prompt || 'Input'}</label>
        <!-- svelte-ignore a11y_autofocus -->
        <input id="stdin-{cell.id}" type={input.password ? 'password' : 'text'} bind:value autofocus onmousedown={(e) => e.stopPropagation()} />
        <kbd>Enter</kbd>
      </form>
    {/if}
  </div>
</div>

<svelte:window onclick={(e) => moreOpen && !(e.target as Element).closest('.more') && (moreOpen = false)} />

<style>
  .cell { display: flex; padding: 6px 0; position: relative; }
  .cell::before { content: ''; position: absolute; left: 6px; top: 6px; bottom: 6px; width: 2px; border-radius: 1px; }
  .selected::before { background: var(--border-strong); }
  .editing::before { background: var(--accent); }
  .tools .label { padding: 0 8px; font-size: 0.85rem; }
  .more { position: relative; }
  .menu { position: absolute; top: calc(100% + 4px); right: 0; z-index: 10; min-width: 190px; padding: 4px; background: var(--popover); border: 1px solid var(--border-strong); border-radius: 6px; box-shadow: 0 8px 24px #0006; }
  .menu button { width: 100%; justify-content: space-between; padding: 5px 8px; border: 0; border-radius: 4px; }
  .menu button:hover { background: var(--hover-strong); }
  .menu kbd { font: 0.8rem var(--mono); color: var(--faint); }
  .gutter { width: 64px; flex: none; display: flex; flex-direction: column; align-items: flex-end; padding: 14px 10px 0 0; gap: 4px; }
  .count { font: 0.885rem var(--mono); color: var(--faint); }
  .selected .count { color: var(--muted); }
  .run { width: 26px; height: 26px; padding: 0; justify-content: center; border-radius: 50%; color: var(--muted); opacity: 0; }
  .cell:hover .run, .selected .run, .run.running { opacity: 1; }
  .run:hover:not(:disabled) { color: var(--accent); }
  .spinner { width: 12px; height: 12px; border-radius: 50%; border: 2px solid var(--accent-soft); border-top-color: var(--accent); animation: spin 0.8s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
  .took { display: flex; align-items: center; gap: 3px; font: 0.77rem var(--mono); color: var(--faint); white-space: nowrap; }
  .took.ok svg { color: var(--ok); }
  .took.failed svg { color: var(--error); }
  .body { flex: 1; min-width: 0; margin-right: 28px; position: relative; }
  .tools { position: absolute; top: -13px; right: 8px; z-index: 5; display: none; align-items: center; gap: 1px; padding: 1px; background: var(--popover); border: 1px solid var(--border-strong); border-radius: 6px; box-shadow: 0 4px 12px #0006; }
  .cell:hover .tools, .selected .tools { display: flex; }
  .tools button { height: 22px; min-width: 24px; padding: 0 5px; justify-content: center; color: var(--muted); border: none; font-size: 0.846rem; }
  .tools button:hover:not(:disabled) { color: var(--fg); background: var(--hover-strong); }
  .tools button:disabled { color: var(--faint); opacity: 0.5; }
  .source { background: var(--editor-bg); border: 1px solid var(--border); border-radius: 6px; transition: border-color 0.12s; }
  .selected .source { border-color: var(--border-strong); }
  .editing .source { border-color: var(--accent-line); }
  .rendered { padding: 6px 14px; border: 1px solid transparent; border-radius: 6px; }
  .stdin { display: flex; align-items: center; gap: 10px; margin-top: 8px; padding: 0 2px; font: 0.962rem var(--mono); }
  .stdin label { color: var(--muted); white-space: pre; }
  .stdin input { flex: 1; max-width: 420px; font-family: var(--mono); }
</style>

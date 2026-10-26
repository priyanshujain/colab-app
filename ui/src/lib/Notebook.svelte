<script lang="ts">
  import { onMount, tick } from 'svelte'
  import type { Cell as CellModel } from '../bindings/Cell'
  import type { CellEdit } from '../bindings/CellEdit'
  import type { CellType } from '../bindings/CellType'
  import type { InputRequest } from '../bindings/InputRequest'
  import type { Api } from './ipc'
  import { applyEdit } from './edit'
  import { offsets, visibleRange } from './virtual'
  import { shown, target, type Lsp } from './lsp'
  import { label, pressed } from './settings'
  import { alerts, settings } from './state.svelte'
  import type { KernelStatus } from '../bindings/KernelStatus'
  import Cell from './Cell.svelte'
  import FileName from './FileName.svelte'
  import Find from './Find.svelte'
  import type { Match } from './find'
  import Icon from './Icon.svelte'
  import { GEAR, MORE, PLUS, RESTART, RUN_ALL, STOP } from './icons'
  import KernelBar from './KernelBar.svelte'
  import Titlebar from './Titlebar.svelte'

  let { api, path, initial, metadata, onopenfile, onsettings, onrenamed }: { api: Api; path: string; initial: CellModel[]; metadata: Record<string, unknown>; onopenfile: (file: string, line: number, character: number) => void; onsettings: () => void; onrenamed: (path: string) => void } = $props()

  // svelte-ignore state_referenced_locally
  let cells = $state(initial)
  let selected = $state(0)
  let editing = $state(false)
  let running = $state(new Set<string>())
  let heights = $state<Record<string, number>>({})
  let scrollTop = $state(0)
  let viewport = $state(0)
  let scroller: HTMLDivElement
  let pendingD = false
  let diagnostics = $state<Record<string, unknown[]>>({})
  let input = $state<InputRequest | null>(null)
  let reveal = $state<{ id: string; line: number; character: number } | null>(null)
  let formatting = new Set<string>()
  let width = $state(1200)
  let menu = $state<'more' | 'format' | null>(null)
  let finding = $state(false)
  let finder = $state<Find>()
  let found = $state<Match[]>([])
  let match = $state<Match | null>(null)
  const none: Match[] = []
  const marks = $derived(Map.groupBy(found, (m) => m.cell))
  let dirty = $state(false)
  let status = $state<KernelStatus>('disconnected')
  let kind = $state<'local' | 'colab'>('local')
  const wide = $derived(width >= 960)
  const live = $derived(status !== 'disconnected' && status !== 'dead')

  const estimate = (c: CellModel) => 40 + c.source.split('\n').length * 19
  const offs = $derived(offsets(cells.map((c) => heights[c.id] ?? estimate(c))))
  const range = $derived(visibleRange(offs, scrollTop, viewport))
  const current = $derived(cells[selected])

  function edit(e: CellEdit) {
    cells = applyEdit(cells, e)
    api.editCell(path, e)
  }

  function show(i: number, enter: boolean) {
    selected = Math.max(0, Math.min(i, cells.length - 1))
    editing = enter
    if (offs[selected] < scroller.scrollTop) scroller.scrollTop = offs[selected]
    else if (offs[selected + 1] > scroller.scrollTop + viewport) scroller.scrollTop = offs[selected + 1] - viewport
  }

  async function select(i: number, enter = false) {
    show(i, enter)
    await tick()
    if (!enter) scroller.focus({ preventScroll: true })
  }

  function insert(index: number, cell_type: CellType = 'code') {
    edit({ kind: 'insert', cell_id: crypto.randomUUID().slice(0, 8), index, cell_type })
    select(index, true)
  }

  function run(advance: boolean) {
    if (!current) return
    if (current.cell_type === 'code') api.runCell(path, current.id)
    if (!advance) return select(selected, current.cell_type === 'code' && editing)
    if (selected === cells.length - 1) insert(cells.length)
    else select(selected + 1, editing && cells[selected + 1].cell_type === 'code')
  }

  function onkey(key: 'Shift-Enter' | 'Mod-Enter' | 'Escape') {
    if (key === 'Escape') select(selected)
    else run(key === 'Shift-Enter')
  }

  function commandKey(e: KeyboardEvent) {
    if (editing || e.target !== scroller || !current) return
    const k = e.key
    const wasD = pendingD
    pendingD = false
    if (pressed(e, settings.keys, 'runCell')) run(false)
    else if (k === 'Enter' && e.shiftKey && !e.metaKey && !e.ctrlKey) run(true)
    else if (k === 'Enter' && !e.metaKey && !e.ctrlKey && !e.altKey) select(selected, true)
    else if (k === 'ArrowDown' || k === 'j') select(selected + 1)
    else if (k === 'ArrowUp' || k === 'k') select(selected - 1)
    else if (k === 'a') insert(selected)
    else if (k === 'b') insert(selected + 1)
    else if (k === 'm') edit({ kind: 'change_type', cell_id: current.id, cell_type: 'markdown' })
    else if (k === 'y') edit({ kind: 'change_type', cell_id: current.id, cell_type: 'code' })
    else if (k === 'd' && wasD) {
      edit({ kind: 'delete', cell_id: current.id })
      select(selected)
    } else if (k === 'd') pendingD = true
    else if (pressed(e, settings.keys, 'formatCell')) current.cell_type === 'code' && format(current.id)
    else if (pressed(e, settings.keys, 'organize')) organize()
    else return
    e.preventDefault()
  }

  function format(id?: string) {
    ;(id ? [id] : cells.map((c) => c.id)).forEach((i) => formatting.add(i))
    ;(id ? api.formatCell(path, id) : api.formatNotebook(path)).catch((e) => (alerts.error = String(e)))
  }

  function organize(id?: string) {
    ;(id ? [id] : cells.map((c) => c.id)).forEach((i) => formatting.add(i))
    api.organizeImports(path, id ?? null).catch((e) => (alerts.error = String(e)))
  }

  async function definition(cell_id: string, line: number, character: number) {
    const t = target(await api.definition({ path, cell_id, line, character }), path)
    if (!t) return
    if ('file' in t) return onopenfile(t.file, t.line, t.character)
    const i = cells.findIndex((c) => c.id === t.cell)
    if (i < 0) return
    await select(i, true)
    reveal = { id: t.cell, line: t.line, character: t.character }
  }

  const lspFor = (cell_id: string): Lsp => ({
    completion: (line, character) => api.completion({ path, cell_id, line, character }),
    hover: (line, character) => api.hover({ path, cell_id, line, character }),
    definition: (line, character) => void definition(cell_id, line, character),
    format: () => format(cell_id),
    organize: () => organize(cell_id),
    codeActions: (line, character) => api.codeActions({ path, cell_id, line, character }),
    prepareRename: (line, character) => api.prepareRename({ path, cell_id, line, character }),
    rename: (line, character, name) => (cells.forEach((c) => formatting.add(c.id)), api.renameSymbol({ path, cell_id, line, character }, name).catch((e) => (alerts.error = String(e)))),
    applyAction: (id) => (cells.forEach((c) => formatting.add(c.id)), api.applyCodeAction(path, id).catch((e) => (alerts.error = String(e)))),
  })

  const remove = (i: number) => (edit({ kind: 'delete', cell_id: cells[i].id }), select(Math.min(i, cells.length - 1)))
  const toggle = (i: number) => edit({ kind: 'change_type', cell_id: cells[i].id, cell_type: cells[i].cell_type === 'code' ? 'markdown' : 'code' })

  function move(i: number, delta: number) {
    edit({ kind: 'move', cell_id: cells[i].id, index: i + delta })
    select(i + delta)
  }

  async function find(e: KeyboardEvent) {
    if (pressed(e, settings.keys, 'runAll')) return (e.preventDefault(), api.runAll(path))
    if (pressed(e, settings.keys, 'formatNotebook')) return (e.preventDefault(), format())
    if (!pressed(e, settings.keys, 'find')) return
    e.preventDefault()
    finding = true
    await tick()
    finder?.focus(document.getSelection()?.toString().split('\n')[0] ?? '')
  }

  function closeFind() {
    finding = false
    found = []
    match = null
    if (editing) return
    scroller.focus({ preventScroll: true })
  }

  const rename = async (name: string) => onrenamed(await api.renameNotebook(path, name))

  function measure(node: HTMLElement, id: string) {
    const ro = new ResizeObserver(() => (heights[id] = node.offsetHeight))
    ro.observe(node)
    return { destroy: () => ro.disconnect() }
  }

  onMount(() => {
    const listening = api.onDiagnostics((e) => {
      if (e.path === path) diagnostics[e.cell_id] = e.diagnostics
    })
    listening.then(() => api.diagnostics(path)).then((list) => list.forEach((e) => (diagnostics[e.cell_id] ??= e.diagnostics)), () => {})
    const subs = [
      api.onCellOutputs((e) => {
        if (e.path !== path) return
        running = new Set(running)
        e.running ? running.add(e.cell_id) : running.delete(e.cell_id)
        if (!e.running && input?.cell_id === e.cell_id) input = null
        cells = cells.map((c) => (c.id === e.cell_id ? { ...c, outputs: e.outputs, execution_count: e.execution_count, metadata: { ...c.metadata, execution: e.execution ?? undefined } } : c))
      }),
      api.onNotebookChanged((e) => {
        if (e.path !== path) return
        const mine = editing && current && !formatting.has(current.id) ? current : undefined
        formatting.clear()
        cells = e.notebook.cells.map((c) => (c.id === mine?.id ? { ...c, source: mine.source } : c))
      }),
      listening,
      api.onInputRequest((e) => {
        if (e.path === path) input = e
      }),
      api.onSaveState((e) => {
        if (e.path === path) dirty = e.dirty
      }),
      api.onAgentFocus(async (e) => {
        if (e.path !== path || editing) return
        await tick()
        const i = cells.findIndex((c) => c.id === e.cell_id)
        if (i >= 0) show(i, false)
      }),
    ]
    return () => subs.forEach((s) => s.then((u) => u()))
  })

  const focusId = $derived(editing ? (current?.id ?? null) : null)
  $effect(() => void api.editCell(path, { kind: 'focus', cell_id: focusId }))
</script>

<Titlebar>
  {#snippet center()}<FileName {path} {dirty} onrename={rename} onmenu={() => api.fileMenu(path)} />{/snippet}
  {#snippet trail()}
    <span class="kernel {status}" title="Kernel {status}"></span>
    <button class="icon-button" onclick={onsettings} title="Settings" aria-label="Settings"><Icon d={GEAR} /></button>
  {/snippet}
</Titlebar>

<div class="actions" bind:clientWidth={width}>
  <button onclick={() => insert(selected + 1)} title="Add code cell (B)"><Icon d={PLUS} size={14} />Code</button>
  {#if wide}<button onclick={() => insert(selected + 1, 'markdown')} title="Add markdown cell"><Icon d={PLUS} size={14} />Markdown</button>{/if}
  <span class="sep"></span>
  <button onclick={() => api.runAll(path)} title="Run all cells ({label(settings.keys.runAll)})"><Icon d={RUN_ALL} size={14} />Run all</button>
  <button onclick={() => api.interrupt(path)} disabled={!live} title="Interrupt the kernel"><Icon d={STOP} size={14} />Interrupt</button>
  {#if wide}
    <button onclick={() => api.restart(path)} disabled={!live} title="Restart the kernel"><Icon d={RESTART} size={14} />Restart</button>
    <span class="sep"></span>
    <div class="more">
      <button class="drop" onclick={() => (menu = menu === 'format' ? null : 'format')} title="Format ({label(settings.keys.formatCell)})" aria-expanded={menu === 'format'}>Format</button>
      {#if menu === 'format'}{@render formats()}{/if}
    </div>
  {:else}
    <div class="more">
      <button class="icon-button" onclick={() => (menu = menu === 'more' ? null : 'more')} title="More actions" aria-label="More actions" aria-expanded={menu === 'more'}><Icon d={MORE} /></button>
      {#if menu === 'more'}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div class="menu" onclick={() => (menu = null)}>
          <button onclick={() => insert(selected + 1, 'markdown')}>Add markdown cell</button>
          <button onclick={() => api.restart(path)} disabled={!live}>Restart kernel</button>
          <button onclick={() => current && format(current.id)} disabled={current?.cell_type !== 'code'}>Format cell</button>
          <button onclick={() => format()}>Format notebook</button>
          <button onclick={() => organize(editing ? current?.id : undefined)}>Organize imports</button>
        </div>
      {/if}
    </div>
  {/if}
  <span class="grow"></span>
  <KernelBar {api} {path} {wide} bind:status bind:kind />
  {#if finding}
    <Find bind:this={finder} sources={cells.map((c) => c.source)} start={selected} onfound={(m) => (found = m)} onreveal={(m) => (show(m.cell, false), (match = { ...m }))} onreplace={(i, source) => edit({ kind: 'source', cell_id: cells[i].id, source })} onclose={closeFind} />
  {/if}
</div>

{#snippet formats()}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="menu" onclick={() => (menu = null)}>
    <button onclick={() => current && format(current.id)} disabled={current?.cell_type !== 'code'}>Format Cell</button>
    <button onclick={() => format()}>Format Notebook</button>
    <button onclick={() => organize(editing ? current?.id : undefined)} title={label(settings.keys.organize)}>Organize Imports</button>
  </div>
{/snippet}

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div class="scroller" tabindex="0" role="list" bind:this={scroller} bind:clientHeight={viewport} onscroll={() => (scrollTop = scroller.scrollTop)} onkeydown={commandKey}>
  <div class="spacer" style:height="{offs[cells.length]}px">
    {#each cells.slice(range[0], range[1]) as cell, j (cell.id)}
      {@const i = range[0] + j}
      <div class="row" style:top="{offs[i]}px" use:measure={cell.id}>
        <Cell
          {cell}
          {path}
          {metadata}
          selected={i === selected}
          editing={editing && i === selected}
          running={running.has(cell.id)}
          lsp={lspFor(cell.id)}
          diagnostics={shown(diagnostics[cell.id] ?? [], settings, kind === 'colab')}
          marks={marks.get(i) ?? none}
          match={match?.cell === i ? match : null}
          reveal={reveal?.id === cell.id ? reveal : null}
          input={input?.cell_id === cell.id ? input : null}
          onselect={() => ((selected = i), (editing = false))}
          onedit={() => ((selected = i), (editing = true))}
          onchange={(source) => edit({ kind: 'source', cell_id: cell.id, source })}
          first={i === 0}
          last={i === cells.length - 1}
          onrun={() => api.runCell(path, cell.id)}
          onmove={(delta) => move(i, delta)}
          ontoggle={() => toggle(i)}
          ondelete={() => remove(i)}
          onreply={(value) => ((input = null), api.sendInput(path, value))}
          {onkey}
        />
      </div>
    {/each}
  </div>
</div>

<svelte:window onkeydown={find} onmousedown={(e) => menu && !(e.target as Element).closest('.more') && (menu = null)} />

<style>
  .actions { position: relative; flex: none; display: flex; align-items: center; gap: 2px; height: 40px; padding: 0 12px; border-bottom: 1px solid var(--border); background: var(--bg); min-width: 0; }
  .actions > button, .drop, .menu button { color: var(--muted); height: 28px; }
  .actions > button:hover:not(:disabled), .drop:hover, .menu button:hover:not(:disabled) { color: var(--fg); border-color: transparent; background: var(--hover-strong); }
  .actions > button:disabled { color: var(--faint); }
  .sep { width: 1px; height: 16px; background: var(--border); margin: 0 6px; flex: none; }
  .grow { flex: 1; min-width: 8px; }
  .kernel { width: 8px; height: 8px; border-radius: 50%; background: var(--faint); margin: 0 8px; flex: none; }
  .kernel.idle { background: var(--ok); }
  .kernel.busy, .kernel.connecting, .kernel.reconnecting { background: var(--warn); animation: pulse 1s ease-in-out infinite; }
  .kernel.dead { background: var(--error); }
  @keyframes pulse { 50% { opacity: 0.35; } }
  .more { position: relative; flex: none; }
  .menu { position: absolute; left: 0; top: calc(100% + 6px); min-width: 190px; z-index: 20; background: var(--popover); border: 1px solid var(--border-strong); border-radius: 8px; padding: 4px; box-shadow: 0 12px 32px #0008; display: flex; flex-direction: column; }
  .menu button { justify-content: flex-start; color: var(--fg); }
  .menu button:disabled { color: var(--faint); }
  .drop::after { content: ''; width: 0; height: 0; border: 4px solid transparent; border-top-color: currentColor; margin: 4px 0 0 -2px; }
  .scroller { flex: 1; overflow-y: auto; outline: none; padding: 12px 0 40vh; }
  .spacer { position: relative; max-width: 1100px; margin: 0 auto; }
  .row { position: absolute; left: 0; right: 0; }
</style>

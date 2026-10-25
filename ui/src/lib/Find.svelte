<script lang="ts">
  import { tick } from 'svelte'
  import { findAll, pattern, replaceAll, replaceAt, type Match } from './find'
  import Icon from './Icon.svelte'
  import { DOWN, UP } from './icons'

  let { sources, start, onfound, onreveal, onreplace, onclose }: {
    sources: string[]
    start: number
    onfound: (matches: Match[]) => void
    onreveal: (m: Match) => void
    onreplace: (cell: number, source: string) => void
    onclose: () => void
  } = $props()

  let query = $state('')
  let replacement = $state('')
  let caseSensitive = $state(false)
  let wholeWord = $state(false)
  let regex = $state(false)
  let replacing = $state(false)
  let index = $state(-1)
  let field: HTMLInputElement
  const re = $derived(pattern(query, { caseSensitive, wholeWord, regex }))
  const matches = $derived(findAll(sources, re))
  const label = $derived(!query ? '' : !matches.length ? 'No results' : `${index >= 0 && index < matches.length ? index + 1 : '?'} of ${matches.length}`)
  $effect(() => onfound(matches))

  function go(i: number) {
    if (!matches.length) return void (index = -1)
    index = (i + matches.length) % matches.length
    onreveal(matches[index])
  }

  const search = () => go(Math.max(0, matches.findIndex((m) => m.cell >= start)))
  const toggle = (set: () => void) => (set(), search(), field.focus())

  export function focus(text: string) {
    if (text) (query = text), search()
    field.focus()
    field.select()
  }

  async function replaceOne() {
    const m = matches[index]
    if (!re || !m) return
    onreplace(m.cell, replaceAt(sources[m.cell], m, re, replacement, regex))
    await tick()
    go(index)
  }

  function replaceEvery() {
    if (!re) return
    new Set(matches.map((m) => m.cell)).forEach((c) => onreplace(c, replaceAll(sources[c], re, replacement, regex)))
  }

  function key(e: KeyboardEvent, enter: () => void) {
    if (e.key === 'Escape') onclose()
    else if (e.key === 'Enter') enter()
    else return
    e.preventDefault()
    e.stopPropagation()
  }
</script>

<div class="find" role="search">
  <button class="expand" class:open={replacing} onclick={() => (replacing = !replacing)} title="Toggle replace" aria-label="Toggle replace">›</button>
  <div class="rows">
    <div class="row">
      <div class="input">
        <input bind:this={field} value={query} oninput={(e) => ((query = e.currentTarget.value), search())} onkeydown={(e) => key(e, () => go(index + (e.shiftKey ? -1 : 1)))} placeholder="Find" aria-label="Find" spellcheck="false" />
        <button class:on={caseSensitive} onclick={() => toggle(() => (caseSensitive = !caseSensitive))} title="Match case" aria-pressed={caseSensitive}>Aa</button>
        <button class="word" class:on={wholeWord} onclick={() => toggle(() => (wholeWord = !wholeWord))} title="Match whole word" aria-pressed={wholeWord}>ab</button>
        <button class:on={regex} onclick={() => toggle(() => (regex = !regex))} title="Use regular expression" aria-pressed={regex}>.*</button>
      </div>
      <span class="count" class:none={query && !matches.length}>{label}</span>
      <button class="icon" onclick={() => go(index - 1)} disabled={!matches.length} title="Previous match (Shift+Enter)" aria-label="Previous match"><Icon d={UP} size={14} /></button>
      <button class="icon" onclick={() => go(index + 1)} disabled={!matches.length} title="Next match (Enter)" aria-label="Next match"><Icon d={DOWN} size={14} /></button>
      <button class="icon" onclick={onclose} title="Close (Escape)" aria-label="Close find"><Icon d="M6 6l12 12M18 6L6 18" size={14} /></button>
    </div>
    {#if replacing}
      <div class="row">
        <div class="input"><input bind:value={replacement} onkeydown={(e) => key(e, replaceOne)} placeholder="Replace" aria-label="Replace" spellcheck="false" /></div>
        <button class="text" onclick={replaceOne} disabled={index < 0 || !matches.length} title="Replace (Enter)">Replace</button>
        <button class="text" onclick={replaceEvery} disabled={!matches.length} title="Replace all">All</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .find { position: absolute; top: calc(100% + 2px); right: 24px; z-index: 30; display: flex; gap: 2px; padding: 4px 6px 4px 2px; background: var(--popover); border: 1px solid var(--border-strong); border-radius: 6px; box-shadow: 0 8px 24px #0007; }
  .rows { display: flex; flex-direction: column; gap: 4px; }
  .row { display: flex; align-items: center; gap: 2px; }
  .input { display: flex; align-items: center; width: 280px; background: var(--editor-bg); border: 1px solid var(--border); border-radius: 4px; padding-right: 2px; }
  .input:focus-within { border-color: var(--accent-line); }
  .input input { flex: 1; min-width: 0; border: none; background: transparent; padding: 3px 6px; font-size: 0.962rem; }
  .input button { height: 20px; min-width: 22px; padding: 0 3px; justify-content: center; font: 0.846rem var(--mono); color: var(--muted); border-radius: 3px; }
  .input button.on { color: var(--fg); background: var(--accent-soft); border-color: var(--accent-line); }
  .word { text-decoration: underline; text-underline-offset: 2px; }
  .count { min-width: 72px; padding: 0 6px; font-size: 0.885rem; color: var(--muted); white-space: nowrap; }
  .count.none { color: var(--error); }
  button { color: var(--muted); }
  .icon { width: 24px; height: 24px; padding: 0; justify-content: center; }
  .text { height: 24px; padding: 0 8px; }
  .expand { width: 18px; padding: 0; justify-content: center; align-self: stretch; font-size: 1.1rem; transition: transform 120ms; }
  .expand.open { transform: rotate(90deg); }
  button:hover:not(:disabled) { color: var(--fg); background: var(--hover-strong); border-color: transparent; }
</style>

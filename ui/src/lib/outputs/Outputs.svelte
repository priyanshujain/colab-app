<script module lang="ts">
  import { onComm } from './comm'
  let queue = Promise.resolve()
  void onComm(m => { queue = queue.then(() => import('./widgets')).then(w => w.handleComm(m)).catch(console.error) })
</script>

<script lang="ts">
  import type { Cell } from '../../bindings/Cell'
  import Output from './Output.svelte'
  let { cell, path, metadata }: { cell: Cell, path: string, metadata?: Record<string, unknown> } = $props()
  const saved = $derived((metadata?.widgets as Record<string, unknown> | undefined)?.['application/vnd.jupyter.widget-state+json'])
  $effect(() => { if (saved) import('./widgets').then(w => w.restoreState(path, saved)) })
</script>

{#if cell.outputs?.length}
  <div class="outputs">
    {#each cell.outputs as output, i (i)}
      <Output {output} {path} />
    {/each}
  </div>
{/if}

<style>
  .outputs {
    --mono: var(--font-mono, 'JetBrains Mono', 'SF Mono', Menlo, Consolas, monospace);
    --out-fg: var(--fg, #d4d4d4);
    --out-bg: var(--bg, #1e1e1e);
    --out-border: var(--border, #333);
    --out-muted: var(--muted, #8a8a8a);
    --out-error: var(--error, #f48771);
    --jp-widgets-color: var(--out-fg);
    --jp-widgets-label-color: var(--out-fg);
    --jp-widgets-readout-color: var(--out-fg);
    --jp-widgets-input-color: var(--out-fg);
    --jp-widgets-input-background-color: var(--out-input);
    --jp-widgets-input-border-color: var(--out-input-border);
    --jp-widgets-font-size: 13px;
    --jp-ui-font-color1: var(--out-fg);
    --jp-layout-color1: var(--out-input);
    --jp-layout-color2: var(--out-layout);
    --jp-layout-color3: var(--out-strong);
    --jp-border-color1: var(--out-strong);
    --jp-widgets-slider-handle-background-color: var(--out-handle);
    --jp-widgets-slider-active-handle-color: var(--accent, #3794ff);
    color: var(--out-fg);
    font-size: var(--ui-font-size, 13px);
    padding: 4px 0;
    min-width: 0;
    max-width: 100%;
    overflow-x: auto;
  }
  .outputs :global(pre.text) { margin: 0; padding: 2px 8px; font: var(--code-size, 12.5px)/1.45 var(--mono); white-space: pre-wrap; word-break: break-word; }
  .outputs :global(pre.stderr) { background: var(--out-stderr); }
  .outputs :global(pre.error) { color: var(--out-fg); }
  .outputs :global(pre.json) { color: var(--out-json); }
  .outputs :global(.more) { margin: 2px 8px; background: none; border: 1px solid var(--out-border); color: var(--out-muted); border-radius: 3px; cursor: pointer; font: 12px var(--mono); }
  .outputs :global(.more:hover) { color: var(--out-fg); }
  .outputs :global(img) { display: block; max-width: 100%; padding: 4px 8px; }
  .outputs :global(.html), .outputs :global(.markdown), .outputs :global(.latex) { padding: 2px 8px; overflow-x: auto; }
  .outputs :global(.html) { font-family: var(--md-font, inherit); font-size: var(--md-size, inherit); }
  .outputs :global(.markdown code) { font-family: var(--mono); }
  .outputs :global(.widget) { padding: 2px 8px; overflow-x: auto; }
  .outputs :global(table.dataframe) { border: none; border-collapse: collapse; font: 12px var(--mono); margin: 4px 0; }
  .outputs :global(.dataframe th), .outputs :global(.dataframe td) { padding: 4px 10px; text-align: right; border: none; border-bottom: 1px solid var(--out-border); }
  .outputs :global(.dataframe thead th) { color: var(--out-fg); font-weight: 600; border-bottom: 1px solid var(--out-strong); }
  .outputs :global(.dataframe tbody th) { color: var(--out-muted); font-weight: 500; }
  .outputs :global(.dataframe tbody tr:nth-child(odd)) { background: var(--out-row-alt); }
  .outputs :global(.dataframe tbody tr:hover) { background: var(--out-row-hover); }
  .outputs :global(.ansi-black-fg) { color: var(--ansi-black); } .outputs :global(.ansi-red-fg) { color: var(--ansi-red); }
  .outputs :global(.ansi-green-fg) { color: var(--ansi-green); } .outputs :global(.ansi-yellow-fg) { color: var(--ansi-yellow); }
  .outputs :global(.ansi-blue-fg) { color: var(--ansi-blue); } .outputs :global(.ansi-magenta-fg) { color: var(--ansi-magenta); }
  .outputs :global(.ansi-cyan-fg) { color: var(--ansi-cyan); } .outputs :global(.ansi-white-fg) { color: var(--ansi-white); }
  .outputs :global(.ansi-bright-black-fg) { color: var(--ansi-bright-black); } .outputs :global(.ansi-bright-red-fg) { color: var(--ansi-bright-red); }
  .outputs :global(.ansi-bright-green-fg) { color: var(--ansi-bright-green); } .outputs :global(.ansi-bright-yellow-fg) { color: var(--ansi-bright-yellow); }
  .outputs :global(.ansi-bright-blue-fg) { color: var(--ansi-bright-blue); } .outputs :global(.ansi-bright-magenta-fg) { color: var(--ansi-bright-magenta); }
  .outputs :global(.ansi-bright-cyan-fg) { color: var(--ansi-bright-cyan); } .outputs :global(.ansi-bright-white-fg) { color: var(--ansi-bright-white); }
  .outputs :global(.ansi-red-bg) { background: var(--ansi-red-bg); } .outputs :global(.ansi-green-bg) { background: var(--ansi-green-bg); }
  .outputs :global(.ansi-yellow-bg) { background: var(--ansi-yellow-bg); } .outputs :global(.ansi-blue-bg) { background: var(--ansi-blue-bg); }
</style>

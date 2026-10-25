<script lang="ts">
  import fixture from './fixture.ipynb?raw'
  import type { Cell } from '../../bindings/Cell'
  import Outputs from './Outputs.svelte'
  const nb = JSON.parse(fixture)
  const join = (v: unknown) => Array.isArray(v) ? v.join('') : v
  const cells: Cell[] = nb.cells.map((c: Cell & { source: unknown }) => ({
    ...c, source: join(c.source),
    outputs: c.outputs?.map(o => 'text' in o ? { ...o, text: join(o.text) } : 'data' in o ? { ...o, data: Object.fromEntries(Object.entries(o.data).map(([k, v]) => [k, join(v)])) } : o),
  }))
</script>

{#each cells as cell (cell.id)}
  <pre class="src">{cell.source}</pre>
  <Outputs {cell} path="fixture" metadata={nb.metadata} />
{/each}

<style>
  .src { white-space: pre-wrap; overflow-wrap: anywhere; font: 12px monospace; color: #888; border-top: 1px solid #333; padding-top: 8px; margin: 8px 0 0; }
</style>

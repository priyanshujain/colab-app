<script lang="ts">
  import { ansiToHtml, collapseCarriageReturns, truncate } from './text'
  let { text, kind = '' }: { text: string, kind?: string } = $props()
  let full = $state(false)
  const clean = $derived(collapseCarriageReturns(text))
  const short = $derived(full ? null : truncate(clean))
</script>

<pre class="text {kind}">{@html ansiToHtml(short ?? clean)}</pre>
{#if short !== null}
  <button class="more" onclick={() => full = true}>Show all ({clean.split('\n').length} lines)</button>
{/if}

<script lang="ts">
  import type { Output } from '../../bindings/Output'
  import Text from './Text.svelte'
  import Frame from './Frame.svelte'
  import Widget from './Widget.svelte'
  import { WIDGET, frameDoc, needsFrame, pick, str } from './mime'
  import { renderMarkdown, renderMath } from './math'
  let { output, path }: { output: Output, path: string } = $props()
  const data = $derived(output.output_type === 'stream' || output.output_type === 'error' ? {} : output.data)
  const mime = $derived(pick(data))
  const value = $derived(mime ? str(data[mime]) : '')
</script>

{#if output.output_type === 'stream'}
  <Text text={output.text} kind={output.name} />
{:else if output.output_type === 'error'}
  <Text text={output.traceback.length ? output.traceback.join('\n') : `${output.ename}: ${output.evalue}`} kind="error" />
{:else if mime === WIDGET}
  <Widget {path} modelId={(data[WIDGET] as { model_id: string }).model_id} fallback={str(data['text/plain'] ?? '')} />
{:else if mime && needsFrame(mime, value)}
  <Frame doc={frameDoc(mime, data[mime])} />
{:else if mime === 'text/html'}
  <div class="html">{@html value}</div>
{:else if mime === 'image/svg+xml'}
  <img src="data:image/svg+xml;charset=utf-8,{encodeURIComponent(value)}" alt="svg output" />
{:else if mime?.startsWith('image/')}
  <img src="data:{mime};base64,{value.replace(/\s/g, '')}" alt="output" />
{:else if mime === 'text/markdown'}
  <div class="markdown">{@html renderMarkdown(value)}</div>
{:else if mime === 'text/latex'}
  <div class="latex">{@html renderMath(value)}</div>
{:else if mime === 'application/json'}
  <pre class="text json">{value}</pre>
{:else if mime === 'text/plain'}
  <Text text={value} />
{/if}

<script lang="ts">
  import { onMount } from 'svelte'
  let { path, modelId, fallback }: { path: string, modelId: string, fallback: string } = $props()
  let el: HTMLDivElement
  let ready = $state(false)
  onMount(() => {
    let dispose: (() => void) | undefined, gone = false
    import('./widgets').then(w => w.renderWidget(path, modelId, el)).then(d => { ready = true; gone ? d() : dispose = d })
    return () => { gone = true; dispose?.() }
  })
</script>

<div class="widget" bind:this={el}></div>
{#if !ready}<pre>{fallback}</pre>{/if}

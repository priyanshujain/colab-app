<script lang="ts">
  let { doc }: { doc: string } = $props()
  let frame: HTMLIFrameElement | undefined = $state()
  let height = $state(0)
  $effect(() => {
    const onMessage = (e: MessageEvent) => {
      if (e.source === frame?.contentWindow && typeof e.data?.frameHeight === 'number') height = e.data.frameHeight
    }
    addEventListener('message', onMessage)
    return () => removeEventListener('message', onMessage)
  })
</script>

<iframe bind:this={frame} srcdoc={doc} sandbox="allow-scripts allow-popups" title="output" style:height="{height}px"></iframe>

<style>
  iframe { display: block; width: 100%; border: 0; background: transparent; }
</style>

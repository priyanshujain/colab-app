<script lang="ts">
  let { value = $bindable(), fallback }: { value: string; fallback: string } = $props()
  const colors = [['', 'Theme'], ['#0a84ff', 'Blue'], ['#bf5af2', 'Purple'], ['#ff375f', 'Pink'], ['#ff453a', 'Red'], ['#ff9f0a', 'Orange'], ['#ffd60a', 'Yellow'], ['#30d158', 'Green'], ['#8e8e93', 'Graphite']]
  const custom = $derived(!colors.some((c) => c[0] === value))
  const name = $derived(custom ? 'Custom' : colors.find((c) => c[0] === value)![1])
</script>

<div class="swatches">
  <div class="row" role="group" aria-label="Accent color">
    {#each colors as [hex, label] (hex)}
      <button type="button" class="swatch" style:--c={hex || fallback} aria-label={label} title={label} aria-pressed={value === hex} onclick={() => (value = hex)}></button>
    {/each}
    <label class="swatch well" class:on={custom} title="Custom"><input type="color" aria-label="Custom accent color" value={(value || fallback).slice(0, 7)} oninput={(e) => (value = e.currentTarget.value)} /></label>
  </div>
  <small>{name}</small>
</div>

<style>
  .swatches { margin-left: auto; display: flex; flex-direction: column; align-items: flex-end; gap: 4px; }
  .row { display: flex; flex-wrap: wrap; justify-content: flex-end; gap: 8px; }
  .swatch { position: relative; width: 20px; height: 20px; padding: 0; border: none; border-radius: 50%; background: var(--c); box-shadow: inset 0 0.5px 0 #ffffff59, inset 0 0 0 0.5px #00000033; cursor: pointer; }
  .swatch:hover:not(:disabled) { background: var(--c); }
  .swatch[aria-pressed='true']::after, .swatch.on::after { content: ''; position: absolute; inset: 7px; border-radius: 50%; background: white; box-shadow: 0 0.5px 1px #00000040; }
  .swatch:focus-visible, .well:has(:focus-visible) { outline: none; box-shadow: var(--ring); }
  .swatch[aria-pressed='true'], .swatch.on { outline: 2px solid color-mix(in srgb, var(--c) 55%, transparent); outline-offset: 1px; }
  .well { background: conic-gradient(#ff453a, #ffd60a, #30d158, #64d2ff, #0a84ff, #bf5af2, #ff453a); }
  .well input { position: absolute; inset: 0; width: 100%; height: 100%; padding: 0; border: none; opacity: 0; cursor: pointer; }
  small { color: var(--muted); font-size: 0.846rem; }
</style>

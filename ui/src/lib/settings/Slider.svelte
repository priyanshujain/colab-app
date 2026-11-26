<script lang="ts">
  let { value = $bindable(), min, max, step = 1, label, unit = '' }: { value: number; min: number; max: number; step?: number; label: string; unit?: string } = $props()
</script>

<div class="slider">
  <input type="range" {min} {max} {step} bind:value aria-label={label} aria-valuetext="{value}{unit}" style:--fill="{((value - min) / (max - min)) * 100}%" />
  <output>{value}{unit}</output>
</div>

<style>
  .slider { display: flex; align-items: center; gap: 12px; width: min(240px, 50%); flex: none; }
  input { flex: 1; min-width: 0; height: 20px; margin: 0; appearance: none; background: none; cursor: pointer; }
  input::-webkit-slider-runnable-track { height: 4px; border-radius: 2px; background: linear-gradient(var(--accent), var(--accent)) 0 / var(--fill) 100% no-repeat, color-mix(in srgb, var(--fg) 16%, transparent); box-shadow: inset 0 0 0 0.5px var(--hairline); }
  input::-moz-range-track { height: 4px; border-radius: 2px; background: linear-gradient(var(--accent), var(--accent)) 0 / var(--fill) 100% no-repeat, color-mix(in srgb, var(--fg) 16%, transparent); }
  input::-webkit-slider-thumb { appearance: none; width: 26px; height: 18px; margin-top: -7px; border-radius: 9px; background: white; box-shadow: 0 0 0 0.5px #00000026, 0 1px 4px #00000059; }
  input::-moz-range-thumb { width: 26px; height: 18px; border: none; border-radius: 9px; background: white; box-shadow: 0 0 0 0.5px #00000026, 0 1px 4px #00000059; }
  input:focus-visible { outline: none; }
  input:focus-visible::-webkit-slider-thumb { box-shadow: var(--ring); }
  input:focus-visible::-moz-range-thumb { box-shadow: var(--ring); }
  output { min-width: 5ch; text-align: right; color: var(--muted); font-variant-numeric: tabular-nums; }
</style>

<script lang="ts">
  let { value = $bindable(), min, max, step = 1, label }: { value: number; min: number; max: number; step?: number; label: string } = $props()
  const digits = $derived((String(step).split('.')[1] ?? '').length)
  const set = (v: number) => (value = Number.isFinite(v) ? +Math.min(max, Math.max(min, Math.round(v / step) * step)).toFixed(digits) : value)

  const commit = (input: HTMLInputElement) => ((input.value = (set(parseFloat(input.value)), value.toFixed(digits))))

  function onkeydown(e: KeyboardEvent & { currentTarget: HTMLInputElement }) {
    const d = ({ ArrowUp: step, ArrowDown: -step, PageUp: step * 10, PageDown: -step * 10 } as Record<string, number>)[e.key]
    if (e.key === 'Enter') commit(e.currentTarget)
    else if (d) set(value + d)
    else return
    e.preventDefault()
  }
</script>

<div class="stepper" role="group" aria-label={label}>
  <button type="button" tabindex="-1" aria-label="Decrease {label}" disabled={value <= min} onclick={() => set(value - step)}>&minus;</button>
  <input inputmode="decimal" role="spinbutton" aria-label={label} aria-valuenow={value} aria-valuemin={min} aria-valuemax={max} value={value.toFixed(digits)} {onkeydown} onchange={(e) => commit(e.currentTarget)} />
  <button type="button" tabindex="-1" aria-label="Increase {label}" disabled={value >= max} onclick={() => set(value + step)}>+</button>
</div>

<style>
  .stepper { display: inline-flex; align-items: center; flex: none; height: 28px; padding: 0 2px; border-radius: 999px; background: var(--glass); box-shadow: var(--glass-rim); }
  .stepper:focus-within { box-shadow: var(--glass-rim), var(--ring); }
  input { width: 4ch; padding: 0 2px; border: none; border-radius: 0; background: none; text-align: center; font-variant-numeric: tabular-nums; outline: none; }
  input:focus { box-shadow: none; }
  button { width: 24px; height: 24px; padding: 0; justify-content: center; border: none; color: var(--fg); font-size: 1.077rem; line-height: 1; }
  button:disabled { color: var(--faint); }
</style>

<script lang="ts" generics="T extends string">
  let { value = $bindable(), options, label }: { value: T; options: [T, string][]; label: string } = $props()

  function onkeydown(e: KeyboardEvent & { currentTarget: HTMLElement }) {
    const d = ({ ArrowLeft: -1, ArrowUp: -1, ArrowRight: 1, ArrowDown: 1 } as Record<string, number>)[e.key]
    if (!d) return
    e.preventDefault()
    const i = (options.findIndex((o) => o[0] === value) + d + options.length) % options.length
    value = options[i][0]
    ;(e.currentTarget.children[i] as HTMLElement).focus()
  }
</script>

<div class="segmented" role="radiogroup" aria-label={label} tabindex="-1" {onkeydown}>
  {#each options as [id, name] (id)}
    <button type="button" role="radio" aria-checked={value === id} tabindex={value === id ? 0 : -1} onclick={() => (value = id)}>{name}</button>
  {/each}
</div>

<style>
  .segmented { margin-left: auto; display: inline-flex; flex: none; padding: 2px; gap: 2px; border-radius: 999px; background: var(--glass); box-shadow: var(--glass-rim); }
  button { min-width: 64px; height: 26px; justify-content: center; padding: 0 12px; border: none; color: var(--muted); font-weight: 500; }
  button:hover:not(:disabled) { color: var(--fg); }
  button[aria-checked='true'], button[aria-checked='true']:hover:not(:disabled) { background: var(--glass-strong); color: var(--fg); box-shadow: var(--glass-rim), 0 1px 3px light-dark(#0000001f, #00000059); }
  button:focus-visible { outline: none; box-shadow: var(--ring); }
  @media (prefers-reduced-motion: no-preference) { button { transition: background-color 120ms, color 120ms; } }
</style>

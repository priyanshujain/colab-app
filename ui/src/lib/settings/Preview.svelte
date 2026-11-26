<script lang="ts">
  import { settings } from '../state.svelte'
  let { sample }: { sample: string } = $props()
</script>

<div class="preview" aria-hidden="true">
  <div class="markdown"><p><strong>Scaling.</strong> Multiply every value by <code>k</code> and keep it above zero.</p></div>
  <div class="cell"><span class="code">{@html sample}<span class="cursor {settings.cursorStyle}" class:blink={settings.cursorBlink}></span></span></div>
</div>

<style>
  .preview { display: flex; flex-direction: column; gap: 8px; padding: 16px var(--pad); overflow: hidden; border-radius: 12px; background: var(--surface); box-shadow: inset 0 0 0 0.5px var(--hairline); }
  .markdown { padding: 0 14px; }
  .cell { position: relative; padding: 8px 14px; border: 1px solid color-mix(in srgb, var(--accent) 60%, transparent); border-radius: var(--radius); background: var(--editor-bg); overflow: hidden; }
  .code { display: block; white-space: pre; font: var(--code-size)/var(--code-line-height) var(--mono); font-variant-ligatures: var(--code-ligatures); color: var(--code-fg); }
  .cursor { display: inline-block; vertical-align: -0.2em; width: 1ch; height: 1.2em; }
  .cursor.bar { width: 2px; background: var(--accent); }
  .cursor.block { background: color-mix(in srgb, var(--accent) 55%, transparent); }
  .cursor.underline { height: 2px; vertical-align: -0.15em; background: var(--accent); }
  @media (prefers-reduced-motion: no-preference) { .blink { animation: blink 1.2s steps(1) infinite; } }
  @keyframes blink { 50% { opacity: 0; } }
</style>

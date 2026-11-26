<script module lang="ts">
  import { themes } from '../themes'
  const groups = (['Dark', 'Light'] as const).map((name) => ({
    name, light: name === 'Light',
    items: Object.entries(themes).filter(([, t]) => !!t.light === (name === 'Light')).map(([id, t]) => ({ id, name: t.name, style: Object.entries(t.vars).map(([k, v]) => `${k}:${v}`).join(';') })),
  }))
</script>

<script lang="ts">
  import { activeTheme } from '../settings'
  import { settings, system } from '../state.svelte'
  let { sample }: { sample: string } = $props()
  const active = $derived(activeTheme(settings, system.dark))

  function pick(id: string, light: boolean) {
    if (light) settings.lightTheme = id
    else settings.theme = id
    if (settings.appearance !== 'auto') settings.appearance = light ? 'light' : 'dark'
  }
</script>

{#each groups as g (g.name)}
  {@const chosen = g.light ? settings.lightTheme : settings.theme}
  {@const shown = settings.appearance === 'auto' || settings.appearance === (g.light ? 'light' : 'dark')}
  <div class="group" role="group" aria-label="{g.name} themes">
    <h4>{g.name}</h4>
    <div class="grid">
      {#each g.items as t (t.id)}
        <button type="button" class="theme" class:on={shown && themes[t.id] === active} class:alt={shown && t.id === chosen && themes[t.id] !== active} aria-pressed={t.id === chosen} onclick={() => pick(t.id, g.light)}>
          <span class="mini" style={t.style}><span class="cell">{@html sample}</span></span>
          <span class="name">{t.name}</span>
        </button>
      {/each}
    </div>
  </div>
{/each}

<style>
  .group + .group { margin-top: 12px; }
  h4 { margin: 0 0 8px; font-size: 0.846rem; font-weight: 500; color: var(--muted); }
  .grid { --pick: var(--accent); display: grid; grid-template-columns: repeat(auto-fill, minmax(128px, 1fr)); gap: 14px 12px; }
  .theme { display: flex; flex-direction: column; align-items: stretch; gap: 7px; min-width: 0; padding: 0; border: none; border-radius: 10px; background: none; white-space: normal; }
  .theme:hover:not(:disabled) { background: none; }
  .mini { position: relative; display: block; aspect-ratio: 16 / 10; padding: 8px; box-sizing: border-box; overflow: hidden; border-radius: 10px; background: var(--bg); box-shadow: inset 0 0 0 0.5px var(--border-strong), 0 1px 3px #00000033; }
  .cell { display: block; height: 100%; padding: 6px 8px; box-sizing: border-box; overflow: hidden; border-radius: 5px; background: var(--editor-bg); box-shadow: inset 0 0 0 1px var(--border); color: var(--code-fg); font: 9px/1.5 var(--mono); font-variant-ligatures: var(--code-ligatures); white-space: pre; text-align: left; }
  .name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; text-align: center; font-size: 0.923rem; color: var(--muted); }
  .theme:hover .name { color: var(--fg); }
  .on .mini { outline: 2.5px solid var(--pick); outline-offset: 2px; }
  .on .mini::after { content: '\2713'; position: absolute; right: 6px; bottom: 6px; width: 18px; height: 18px; border-radius: 50%; background: var(--pick); color: white; font: 700 11px/18px var(--sans); text-align: center; box-shadow: 0 1px 3px #00000059; }
  .on .name { color: var(--fg); font-weight: 600; }
  .alt .mini { outline: 2px dashed color-mix(in srgb, var(--pick) 60%, transparent); outline-offset: 2px; }
  .alt .name { color: var(--fg); }
  .theme:focus-visible { outline: none; box-shadow: none; }
  .theme:focus-visible .mini { box-shadow: inset 0 0 0 0.5px var(--border-strong), var(--ring); }
</style>

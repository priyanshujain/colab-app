<script lang="ts">
  import type { Api } from './ipc'
  import { account, settings } from './state.svelte'
  import Icon from './Icon.svelte'
  import { BACK } from './icons'
  import Titlebar from './Titlebar.svelte'
  import { themes } from './themes'
  import FontPicker from './FontPicker.svelte'
  import { actions, combo, defaults, label, type Action } from './settings'
  import { onMount } from 'svelte'

  let { api, onback }: { api: Api; onback: () => void } = $props()
  const sections = [
    ['appearance', 'Appearance', 'Theme and fonts for the interface, markdown and code.'],
    ['editor', 'Editor', 'How code and markdown cells behave while you edit.'],
    ['keyboard', 'Keyboard', 'Click a shortcut, then press the new key combination.'],
    ['account', 'Account', 'Your Google account for Colab runtimes.'],
    ['about', 'About', 'Version information.'],
  ] as const
  let section = $state<(typeof sections)[number][0]>('appearance')
  const current = $derived(sections.find((s) => s[0] === section)!)
  let busy = $state(false)
  let error = $state('')
  let recording = $state<Action | null>(null)
  let version = $state('')
  let mono: string[] | null = $state(null)
  let fonts: string[] | null = $state(null)
  onMount(() => {
    api.listMonospaceFonts().then((f) => (mono = f), () => (mono = [settings.codeFont]))
    api.listFonts().then((f) => (fonts = f), () => (fonts = ['Inter']))
    api.version().then((v) => (version = v), () => {})
  })

  async function signIn() {
    busy = true
    error = ''
    try {
      account.auth = await api.signIn(settings.clientId.trim() || null, settings.clientSecret.trim() || null)
    } catch (e) {
      error = String(e)
    }
    busy = false
  }

  const signOut = () => api.signOut().then(() => (account.auth = { state: 'signed_out' }), (e) => (error = String(e)))

  function key(e: KeyboardEvent) {
    if (!recording) return e.key === 'Escape' && onback()
    e.preventDefault()
    e.stopPropagation()
    const c = e.key === 'Escape' ? null : combo(e)
    if (c) settings.keys[recording] = c
    if (c || e.key === 'Escape') recording = null
  }
</script>

<svelte:window onkeydowncapture={key} />

<Titlebar>
  {#snippet lead()}<button class="back" onclick={onback} title="Back (Esc)"><Icon d={BACK} size={14} />Back</button>{/snippet}
  {#snippet center()}<span class="title" data-tauri-drag-region>Settings</span>{/snippet}
</Titlebar>

<div class="layout">
  <nav aria-label="Settings sections">
    {#each sections as [id, name] (id)}
      <button class:active={section === id} aria-current={section === id} onclick={() => (section = id)}>{name}</button>
    {/each}
  </nav>
  <main>
    <h1>{current[1]}</h1>
    <p class="lead">{current[2]}</p>
    {#if section === 'appearance'}
      <div class="card">
        <h3>Theme</h3>
        <label class="row"><span>Color theme</span><select bind:value={settings.theme}>{#each Object.entries(themes) as [id, t] (id)}<option value={id}>{t.name}</option>{/each}</select></label>
      </div>
      <div class="card">
        <h3>Interface</h3>
        <div class="row"><span>Font family</span><FontPicker bind:value={settings.interfaceFont} {fonts} /></div>
        <label class="row"><span>Font size</span><input type="number" min="10" max="20" bind:value={settings.interfaceSize} /></label>
      </div>
      <div class="card">
        <h3>Markdown</h3>
        <div class="row"><span>Font family</span><FontPicker bind:value={settings.markdownFont} {fonts} /></div>
        <label class="row"><span>Font size</span><input type="number" min="10" max="28" bind:value={settings.markdownSize} /></label>
      </div>
      <div class="card">
        <h3>Code</h3>
        <div class="row"><span>Font family</span><FontPicker bind:value={settings.codeFont} fonts={mono} /></div>
        <label class="row"><span>Font size</span><input type="number" min="8" max="32" bind:value={settings.codeSize} /></label>
      </div>
    {:else if section === 'editor'}
      <div class="card">
        <h3>Code</h3>
        <label class="row"><span>Line height</span><input type="number" min="1" max="3" step="0.1" bind:value={settings.lineHeight} /></label>
        <label class="row"><span>Tab size</span><input type="number" min="1" max="8" bind:value={settings.tabSize} /></label>
        <label class="row"><span>Font ligatures</span><input type="checkbox" bind:checked={settings.ligatures} /></label>
        <label class="row"><span>Line numbers</span><input type="checkbox" bind:checked={settings.lineNumbers} /></label>
        <label class="row"><span>Word wrap</span><input type="checkbox" bind:checked={settings.wordWrap} /></label>
      </div>
      <div class="card">
        <h3>Markdown</h3>
        <label class="row"><span>Double click to edit markdown<small>Otherwise use the pencil button or Enter</small></span><input type="checkbox" bind:checked={settings.dblclickMarkdown} /></label>
      </div>
    {:else if section === 'keyboard'}
      <div class="card">
        <h3>Shortcuts</h3>
        {#each actions as [id, name] (id)}
          <div class="row">
            <span>{name}</span>
            <div class="keys">
              {#if settings.keys[id] !== defaults.keys[id]}<button class="reset" onclick={() => (settings.keys[id] = defaults.keys[id])} title="Reset to {label(defaults.keys[id])}">Reset</button>{/if}
              <button class="combo" class:recording={recording === id} onclick={() => (recording = recording === id ? null : id)} aria-label="{name} shortcut">{recording === id ? 'Press keys...' : label(settings.keys[id])}</button>
            </div>
          </div>
        {/each}
      </div>
    {:else if section === 'account'}
      <div class="card">
        <h3>Google</h3>
        <div class="row">
          {#if account.auth.state === 'signed_in'}
            <span>Signed in as <strong>{account.auth.email}</strong></span>
            <button class="outline" onclick={signOut}>Sign out</button>
          {:else}
            <span class="muted">Not signed in to Colab</span>
            <button class="primary" onclick={signIn} disabled={busy}>{busy ? 'Waiting for browser...' : 'Sign in with Google'}</button>
          {/if}
        </div>
        {#if error || account.auth.state === 'client_rejected'}
          <p class="error">{error || (account.auth.state === 'client_rejected' && `${account.auth.message} Set your own OAuth client below and sign in again.`)}</p>
        {/if}
      </div>
      <div class="card">
        <h3>Custom OAuth client</h3>
        <p class="muted">Optional. Use your own desktop OAuth client if the built in one stops working. Leave empty for the default.</p>
        <label class="row"><span>Client id</span><input class="mono" bind:value={settings.clientId} placeholder="1234-abc.apps.googleusercontent.com" /></label>
        <label class="row"><span>Client secret</span><input class="mono" type="password" bind:value={settings.clientSecret} /></label>
      </div>
    {:else}
      <div class="card">
        <h3>Colab Notebook</h3>
        <div class="row"><span>Version</span><span class="muted mono">{version || 'unknown'}</span></div>
      </div>
    {/if}
  </main>
</div>

<style>
  .title { font-weight: 500; }
  .back { color: var(--muted); height: 28px; padding-left: 5px; }
  .back:hover:not(:disabled) { color: var(--fg); border-color: transparent; background: var(--hover-strong); }
  .layout { flex: 1; min-height: 0; display: flex; }
  nav { width: 200px; flex: none; display: flex; flex-direction: column; gap: 2px; padding: 20px 10px; border-right: 1px solid var(--border); overflow-y: auto; }
  nav button { justify-content: flex-start; height: 30px; color: var(--muted); border-color: transparent; }
  nav button:hover:not(:disabled) { color: var(--fg); background: var(--hover-strong); border-color: transparent; }
  nav button.active { color: var(--fg); background: var(--hover-strong); font-weight: 500; }
  main { flex: 1; min-width: 0; overflow-y: auto; padding: 28px 36px 60px; }
  main > * { max-width: 680px; }
  h1 { font-size: 1.692rem; font-weight: 600; margin: 0 0 4px; }
  .lead { color: var(--muted); margin: 0 0 22px; }
  .card { border: 1px solid var(--border); border-radius: 10px; background: var(--editor-bg); padding: 4px 16px; margin-bottom: 16px; }
  h3 { font-size: 0.769rem; text-transform: uppercase; letter-spacing: 0.07em; color: var(--faint); font-weight: 600; margin: 10px 0 2px; }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 10px 0; min-height: 32px; }
  .row + .row { border-top: 1px solid var(--border); }
  .row > span { display: flex; flex-direction: column; gap: 2px; }
  small { color: var(--faint); font-size: 0.846rem; }
  .row input:not([type='checkbox']), .row select { width: min(260px, 50%); box-sizing: border-box; }
  .keys { display: flex; gap: 6px; align-items: center; }
  .combo { min-width: 120px; justify-content: center; font-family: var(--mono); font-size: 0.885rem; }
  .combo.recording { border-color: var(--accent); color: var(--accent); }
  .reset { color: var(--muted); border-color: transparent; font-size: 0.885rem; }
  .mono { font-family: var(--mono); font-size: 0.885rem; }
  .muted { color: var(--muted); }
  p { margin: 8px 0; font-size: 0.923rem; }
  .error { color: var(--error); }
  input[type='number'] { background: var(--bg); border: 1px solid var(--border); border-radius: 5px; padding: 5px 9px; outline: none; }
  @media (max-width: 760px) {
    nav { width: 140px; padding: 16px 6px; }
    main { padding: 22px 18px 48px; }
    .combo { min-width: 90px; }
  }
</style>

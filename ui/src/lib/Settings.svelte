<script lang="ts">
  import type { Api, Host, HostStatus, TailscaleStatus } from './ipc'
  import { account, settings } from './state.svelte'
  import Icon from './Icon.svelte'
  import { BACK, CODE, FORWARD, GLOBE, INFO, KEY, KEYBOARD, PALETTE, PERSON, SEARCH, SERVER } from './icons'
  import Titlebar from './Titlebar.svelte'
  import { themes } from './themes'
  import FontPicker from './FontPicker.svelte'
  import { actions, combo, defaults, label, type Action } from './settings'
  import { onMount } from 'svelte'

  let { api, onback }: { api: Api; onback: () => void } = $props()
  const sections = [
    ['server', 'Server', 'When Tailscale is on, your other devices on the same Tailscale network can open notebooks and run kernels on this Mac.', SERVER, ''],
    ['appearance', 'Appearance', 'Theme and fonts for the interface, markdown and code.', PALETTE, '#bf5af2'],
    ['editor', 'Editor', 'How code and markdown cells behave while you edit.', CODE, '#0a84ff'],
    ['keyboard', 'Keyboard', 'Click a shortcut, then press the new key combination.', KEYBOARD, '#8e8e93'],
    ['account', 'Account', 'Your Google account for Colab runtimes.', PERSON, '#30d158'],
    ['hosts', 'Remote Hosts', 'Machines on your Tailscale network that run colab-server with Tailscale turned on. The list is shared with colab-server hosts.', GLOBE, '#5e5ce6'],
    ['secrets', 'Secrets', 'Saved in your macOS Keychain. Every notebook, local or Colab, gets them as environment variables and through google.colab.userdata.get.', KEY, '#ff9f0a'],
    ['about', 'About', 'Version information.', INFO, '#636366'],
  ] as const
  type Section = (typeof sections)[number][0]
  let trail = $state<Section[]>(['server'])
  let at = $state(0)
  const section = $derived(trail[at])
  const current = $derived(sections.find((s) => s[0] === section)!)
  let query = $state('')
  const shown = $derived(sections.filter((s) => s[1].toLowerCase().includes(query.trim().toLowerCase())))
  let busy = $state(false)
  let error = $state('')
  let recording = $state<Action | null>(null)
  let version = $state('')
  let mono: string[] | null = $state(null)
  let fonts: string[] | null = $state(null)
  let secrets = $state<{ name: string; saved: string; value: string; shown: boolean }[]>([])
  let secretError = $state('')
  let hosts = $state<Host[]>([])
  let statuses = $state<HostStatus[]>([])
  let hostError = $state('')
  let draft = $state({ label: '', address: '' })
  let adding = $state(false)
  let info = $state<string | null>(null)
  let reachable = $state<boolean | null>(null)
  let server = $state('')
  let serverBusy = $state(false)
  let serverError = $state('')
  let ts = $state<TailscaleStatus>({ state: 'off', hostname: '' })
  let hostname = $state('')
  let tsBusy = $state(false)
  let tsError = $state('')
  let agentNotebooks = $state(false)
  let agentError = $state('')
  const tsLabels = { off: 'Off', starting: 'Starting...', needs_login: 'Waiting for you to sign in', running: 'Running', error: 'Error' }

  function go(id: Section) {
    if (id === section) return
    trail = [...trail.slice(0, at + 1), id]
    at = trail.length - 1
    info = null
  }

  function apply(s: TailscaleStatus) {
    if (s.state !== 'off' || !hostname) hostname = s.hostname
    ts = s
    reachable = true
  }

  onMount(() => {
    api.listMonospaceFonts().then((f) => (mono = f), () => (mono = [settings.codeFont]))
    api.listFonts().then((f) => (fonts = f), () => (fonts = ['Inter']))
    api.version().then((v) => (version = v), () => {})
    api.openAgentNotebooks().then((on) => (agentNotebooks = on), (e) => (agentError = String(e)))
    api.listHosts().then((h) => (hosts = h), (e) => (hostError = String(e)))
    api.listSecrets().then((s) => (secrets = Object.entries(s).map(([name, value]) => ({ name, saved: name, value, shown: false }))), (e) => (secretError = String(e)))
    const unlisten = api.onTailscaleStatus(apply)
    return () => unlisten.then((u) => u())
  })

  let polling: Promise<unknown> = Promise.resolve()
  $effect(() => {
    const open = section === 'hosts'
    let timer: ReturnType<typeof setTimeout> | undefined
    let stop = false
    const tick = () => (polling = polling.then((): unknown => stop || Promise.all([
      api.tailscaleStatus().then(apply, () => (reachable = false)),
      api.serverStatus().then((s) => (server = s), () => {}),
      open && api.hostStatus().then((s) => (statuses = s), () => {}),
    ]))).then(() => void (stop || (timer = setTimeout(tick, 3000))))
    tick()
    return () => ((stop = true), clearTimeout(timer))
  })

  async function tailscale(change: Promise<TailscaleStatus>, toggle?: HTMLInputElement) {
    tsBusy = true
    tsError = ''
    try {
      apply(await change)
    } catch (e) {
      tsError = String(e)
    }
    if (toggle) toggle.checked = ts.state !== 'off'
    tsBusy = false
  }

  async function power(change: () => Promise<void>) {
    serverBusy = true
    serverError = ''
    await change().catch((e) => (serverError = String(e)))
    server = await api.serverStatus().catch(() => server)
    serverBusy = false
  }

  const hostState = (s?: HostStatus) => !s ? 'Checking...' : !s.available ? 'Unavailable' : s.path === 'relay' ? 'Available (relay)' : 'Available'

  async function saveSecret(s: (typeof secrets)[number]) {
    const name = s.name.trim()
    if (!name) return
    secretError = ''
    try {
      if (s.saved && s.saved !== name) await api.deleteSecret(s.saved)
      await api.setSecret(name, s.value)
      s.saved = s.name = name
    } catch (e) {
      secretError = String(e)
    }
  }

  async function deleteSecret(i: number) {
    const s = secrets[i]
    secretError = ''
    try {
      if (s.saved) await api.deleteSecret(s.saved)
      secrets.splice(i, 1)
    } catch (e) {
      secretError = String(e)
    }
  }

  async function editHosts(change: Promise<Host[]>) {
    hostError = ''
    try {
      hosts = await change
      draft = { label: '', address: '' }
      adding = false
      info = null
    } catch (e) {
      hostError = String(e)
    }
  }

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
    if (!recording) {
      if (e.key !== 'Escape') return
      if (adding) return ((adding = false), (hostError = ''))
      if (info) return (info = null)
      return onback()
    }
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
    <label class="search"><Icon d={SEARCH} size={13} /><input bind:value={query} placeholder="Search" aria-label="Search settings" /></label>
    {#each shown as [id, name, , icon, color] (id)}
      <button class:active={section === id} class:server={id === 'server'} aria-current={section === id} onclick={() => go(id)}>
        {#if id === 'server'}
          <span class="avatar"><Icon d={icon} size={16} /></span>
          <span class="who">Server<small>{server || 'Checking...'}</small></span>
        {:else}
          <span class="badge" style:background={color}><Icon d={icon} size={13} /></span>{name}
        {/if}
      </button>
    {/each}
  </nav>
  <main>
    <header class="head">
      <div class="pill">
        <button onclick={() => at--} disabled={at === 0} aria-label="Back"><Icon d={BACK} size={15} /></button>
        <button onclick={() => at++} disabled={at === trail.length - 1} aria-label="Forward"><Icon d={FORWARD} size={15} /></button>
      </div>
      <h1>{current[1]}</h1>
    </header>
    {#if section === 'server'}
      <div class="card hero">
        <span class="avatar big"><Icon d={SERVER} size={26} /></span>
        <span class="who"><strong>Colab Notebook Server</strong><small>Version {version || 'unknown'}</small></span>
        <span class="state"><i class:ok={server === 'Running'} class:off={server === 'Stopped'}></i>{server || 'Checking...'}</span>
      </div>
      <div class="end">
        {#if server === 'Stopped'}
          <button class="outline" onclick={() => power(api.startServer)} disabled={serverBusy}>Start Server</button>
        {:else}
          <button class="outline" onclick={() => power(() => api.stopServer(true))} disabled={serverBusy || !server}>Restart Server</button>
          <button class="outline" onclick={() => power(() => api.stopServer(false))} disabled={serverBusy || !server}>Stop Server</button>
        {/if}
      </div>
      {#if serverError}<p class="error">{serverError}</p>{/if}
      <h3>Tailscale</h3>
      <div class="card">
        <label class="row"><span>Tailscale<small>{tsLabels[ts.state]}</small></span><input type="checkbox" checked={ts.state !== 'off'} disabled={tsBusy || !reachable} onchange={(e) => tailscale(e.currentTarget.checked ? api.tailscaleEnable(hostname.trim() || undefined) : api.tailscaleDisable(), e.currentTarget)} /></label>
        <label class="row"><span>Device name<small>How this Mac appears to your other devices</small></span>{#if ts.state === 'off'}<input bind:value={hostname} spellcheck="false" />{:else}<span class="value">{ts.hostname}</span>{/if}</label>
        {#if ts.state === 'needs_login'}
          <div class="row"><span>Sign in to Tailscale<small>Opens your browser</small></span><button class="outline" onclick={() => api.openUrl(ts.auth_url!).catch((e) => (tsError = String(e)))} disabled={!ts.auth_url}>Sign in...</button></div>
        {:else if ts.state === 'running'}
          <div class="row"><span>Account<small>{ts.login_name}</small></span><button class="outline" onclick={() => tailscale(api.tailscaleLogout())} disabled={tsBusy}>Sign out</button></div>
          <div class="row"><span>DNS name</span><span class="value mono">{ts.dns_name}</span></div>
        {/if}
        {#if tsError || ts.error}<p class="error">{tsError || ts.error}</p>{/if}
      </div>
    {:else if section === 'appearance'}
      <h3>Theme</h3>
      <div class="card">
        <label class="row"><span>Color theme</span><select bind:value={settings.theme}>{#each Object.entries(themes) as [id, t] (id)}<option value={id}>{t.name}</option>{/each}</select></label>
      </div>
      <h3>Interface</h3>
      <div class="card">
        <div class="row"><span>Font family</span><FontPicker bind:value={settings.interfaceFont} {fonts} /></div>
        <label class="row"><span>Font size</span><input type="number" min="10" max="20" bind:value={settings.interfaceSize} /></label>
        <label class="row"><span>Open notebooks that agents use<small>Shows a window when an agent opens a notebook, and closes it when the agent is done</small></span><input type="checkbox" checked={agentNotebooks} onchange={(e) => api.setOpenAgentNotebooks((agentNotebooks = e.currentTarget.checked)).catch((err) => (agentError = String(err)))} /></label>
        {#if agentError}<p class="error">{agentError}</p>{/if}
      </div>
      <h3>Markdown</h3>
      <div class="card">
        <div class="row"><span>Font family</span><FontPicker bind:value={settings.markdownFont} {fonts} /></div>
        <label class="row"><span>Font size</span><input type="number" min="10" max="28" bind:value={settings.markdownSize} /></label>
      </div>
      <h3>Code</h3>
      <div class="card">
        <div class="row"><span>Font family</span><FontPicker bind:value={settings.codeFont} fonts={mono} /></div>
        <label class="row"><span>Font size</span><input type="number" min="8" max="32" bind:value={settings.codeSize} /></label>
      </div>
    {:else if section === 'editor'}
      <h3>Code</h3>
      <div class="card">
        <label class="row"><span>Line height</span><input type="number" min="1" max="3" step="0.1" bind:value={settings.lineHeight} /></label>
        <label class="row"><span>Tab size</span><input type="number" min="1" max="8" bind:value={settings.tabSize} /></label>
        <label class="row"><span>Font ligatures</span><input type="checkbox" bind:checked={settings.ligatures} /></label>
        <label class="row"><span>Line numbers</span><input type="checkbox" bind:checked={settings.lineNumbers} /></label>
        <label class="row"><span>Word wrap</span><input type="checkbox" bind:checked={settings.wordWrap} /></label>
      </div>
      <h3>Diagnostics</h3>
      <div class="card">
        <label class="row"><span>Lint warnings<small>Ruff, using your project's ruff config</small></span><input type="checkbox" bind:checked={settings.lintDiagnostics} /></label>
        <label class="row"><span>Type errors<small>ty. Missing imports are not shown on Colab kernels, since packages live on the runtime</small></span><input type="checkbox" bind:checked={settings.typeDiagnostics} /></label>
      </div>
      <h3>Markdown</h3>
      <div class="card">
        <label class="row"><span>Double click to edit markdown<small>Otherwise use the pencil button or Enter</small></span><input type="checkbox" bind:checked={settings.dblclickMarkdown} /></label>
      </div>
    {:else if section === 'keyboard'}
      <h3>Shortcuts</h3>
      <div class="card">
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
      <h3>Google</h3>
      <div class="card">
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
      <h3>Custom OAuth client</h3>
      <div class="card">
        <p class="muted">Optional. Use your own desktop OAuth client if the built in one stops working. Leave empty for the default.</p>
        <label class="row"><span>Client id</span><input class="mono" bind:value={settings.clientId} placeholder="1234-abc.apps.googleusercontent.com" /></label>
        <label class="row"><span>Client secret</span><input class="mono" type="password" bind:value={settings.clientSecret} /></label>
      </div>
    {:else if section === 'hosts'}
      <div class="hosts">
        {#each hosts as h (h.label)}
          {@const s = statuses.find((x) => x.label === h.label)}
          <div class="card host">
            <span class="globe"><Icon d={GLOBE} size={18} /></span>
            <span class="who"><strong>{h.label}</strong><small>{h.address}</small></span>
            {#if s?.kernels.length}<span class="muted kernel" title={s.kernels.map((k) => `${k.id} · ${k.label}`).join(', ')}>{s.kernels.map((k) => (k.label ? `${k.id} · ${k.label}` : k.id)).join(', ')}</span>{/if}
            <span class="muted">{hostState(s)}</span>
            <button class="icon-button" class:open={info === h.label} onclick={() => (info = info === h.label ? null : h.label)} aria-label="Details for {h.label}"><Icon d={INFO} size={16} /></button>
            {#if info === h.label}
              <button class="scrim" onclick={() => (info = null)} aria-label="Close details"></button>
              <div class="popover" role="dialog" aria-label="{h.label} details">
                <dl>
                  <dt>Address</dt><dd class="mono">{h.address}</dd>
                  <dt>Path</dt><dd>{s?.path === 'direct' ? 'Direct' : s?.path === 'relay' ? 'Relay' : 'None'}</dd>
                  <dt>Latency</dt><dd>{s?.latency_ms != null ? `${s.latency_ms} ms` : 'Unknown'}</dd>
                </dl>
                <button class="outline danger" onclick={() => editHosts(api.removeHost(h.label))}>Remove</button>
              </div>
            {/if}
          </div>
        {:else}
          <div class="card empty muted">No remote hosts</div>
        {/each}
      </div>
      <div class="end"><button class="outline" onclick={() => ((adding = true), (hostError = ''))}>Add Remote Host...</button></div>
      {#if hostError && !adding}<p class="error">{hostError}</p>{/if}
    {:else if section === 'secrets'}
      <div class="card">
        {#each secrets as s, i (i)}
          <div class="row secret">
            <input class="mono" bind:value={s.name} onchange={() => saveSecret(s)} placeholder="NAME" aria-label="Name" />
            <input class="mono" type={s.shown ? 'text' : 'password'} bind:value={s.value} onchange={() => saveSecret(s)} placeholder="Value" aria-label="Value" />
            <button onclick={() => (s.shown = !s.shown)}>{s.shown ? 'Hide' : 'Show'}</button>
            <button onclick={() => deleteSecret(i)}>Delete</button>
          </div>
        {:else}
          <p class="muted">No secrets yet.</p>
        {/each}
        <div class="row"><button class="outline" onclick={() => secrets.push({ name: '', saved: '', value: '', shown: false })}>Add new secret</button></div>
        {#if secretError}<p class="error">{secretError}</p>{/if}
      </div>
    {:else}
      <h3>Colab Notebook</h3>
      <div class="card">
        <div class="row"><span>Version</span><span class="muted mono">{version || 'unknown'}</span></div>
      </div>
    {/if}
    <p class="foot"><Icon d={INFO} size={13} />{current[2]}</p>
  </main>
</div>

{#if adding}
  <div class="backdrop" role="dialog" aria-modal="true" aria-label="Add Remote Host">
    <form class="sheet" onsubmit={(e) => (e.preventDefault(), editHosts(api.addHost(draft.label.trim(), draft.address.trim())))}>
      <strong>Add Remote Host</strong>
      <small>A machine on your Tailscale network that runs colab-server.</small>
      <label class="field"><span>Name</span><input bind:value={draft.label} placeholder="anton" spellcheck="false" {@attach (n) => n.focus()} /></label>
      <label class="field"><span>Address</span><input class="mono" bind:value={draft.address} placeholder="anton-colab" spellcheck="false" /></label>
      {#if hostError}<p class="error">{hostError}</p>{/if}
      <div class="actions">
        <button type="button" class="outline" onclick={() => ((adding = false), (hostError = ''))}>Cancel</button>
        <button class="primary" disabled={!draft.label.trim() || !draft.address.trim()}>Add</button>
      </div>
    </form>
  </div>
{/if}

<style>
  .title { font-weight: 500; }
  .back { color: var(--muted); height: 28px; padding-left: 5px; }
  .back:hover:not(:disabled) { color: var(--fg); border-color: transparent; background: var(--hover-strong); }
  .layout { flex: 1; min-height: 0; display: flex; --card: color-mix(in srgb, var(--fg) 4%, transparent); --line: color-mix(in srgb, var(--fg) 8%, transparent); }
  nav { width: 220px; flex: none; display: flex; flex-direction: column; gap: 1px; padding: 12px 10px; background: var(--panel); border-right: 1px solid var(--border); overflow-y: auto; }
  .search { display: flex; align-items: center; gap: 6px; height: 28px; padding: 0 8px; margin-bottom: 8px; border-radius: 7px; background: var(--card); color: var(--muted); }
  .search input { flex: 1; min-width: 0; border: none; background: none; padding: 0; outline: none; }
  nav button { justify-content: flex-start; gap: 8px; height: 30px; padding: 0 6px; border-radius: 7px; border-color: transparent; }
  nav button:hover:not(:disabled) { background: var(--hover-strong); border-color: transparent; }
  nav button.active, nav button.active:hover:not(:disabled) { background: color-mix(in srgb, var(--fg) 11%, transparent); }
  nav button.server { height: 48px; margin-bottom: 10px; }
  .badge { display: grid; place-items: center; width: 22px; height: 22px; border-radius: 6px; color: #fff; flex: none; }
  .avatar { display: grid; place-items: center; width: 32px; height: 32px; border-radius: 50%; color: #fff; flex: none; background: linear-gradient(#8e8e93, #48484a); }
  .avatar.big { width: 52px; height: 52px; }
  .who { display: flex; flex-direction: column; min-width: 0; text-align: left; line-height: 1.3; }
  .who small, .who strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  main { flex: 1; min-width: 0; overflow-y: auto; padding: 14px 32px 60px; }
  main > * { max-width: 660px; box-sizing: border-box; }
  .head { display: flex; align-items: center; gap: 12px; margin-bottom: 18px; }
  .pill { display: flex; border-radius: 14px; background: var(--card); border: 1px solid var(--line); }
  .pill button { width: 30px; height: 26px; padding: 0; justify-content: center; border: none; border-radius: 14px; color: var(--muted); }
  .pill button:hover:not(:disabled) { border: none; color: var(--fg); }
  h1 { font-size: 1.077rem; font-weight: 600; margin: 0; }
  h3 { font-size: 0.923rem; font-weight: 600; margin: 22px 2px 8px; }
  .card { border: 1px solid var(--line); border-radius: 10px; background: var(--card); padding: 0 14px; position: relative; }
  .hero { display: flex; align-items: center; gap: 14px; padding: 14px; }
  .hero strong { font-size: 1.077rem; }
  .hero .who { flex: 1; }
  .state { display: flex; align-items: center; gap: 6px; color: var(--muted); }
  .state i { width: 8px; height: 8px; border-radius: 50%; background: var(--warn); }
  .state i.ok { background: var(--ok); }
  .state i.off { background: var(--muted); }
  .row { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 10px 0; min-height: 26px; }
  .row + .row { border-top: 1px solid var(--line); }
  .row > span { display: flex; flex-direction: column; gap: 2px; }
  small { color: var(--muted); font-size: 0.846rem; }
  .value { color: var(--muted); }
  .row input:not([type='checkbox']), .row select { width: min(260px, 50%); box-sizing: border-box; }
  .row input[type='checkbox'] { appearance: none; position: relative; flex: none; width: 32px; height: 18px; margin: 0; border-radius: 9px; background: color-mix(in srgb, var(--fg) 20%, transparent); cursor: pointer; transition: background 150ms; }
  .row input[type='checkbox']::before { content: ''; position: absolute; top: 2px; left: 2px; width: 14px; height: 14px; border-radius: 50%; background: #fff; box-shadow: 0 1px 2px #0006; transition: transform 150ms; }
  .row input[type='checkbox']:checked { background: var(--accent); }
  .row input[type='checkbox']:checked::before { transform: translateX(14px); }
  .row input:disabled { opacity: 0.5; cursor: default; }
  .row.secret { justify-content: flex-start; gap: 8px; }
  .row.secret input { flex: 1; width: auto; min-width: 0; }
  .hosts { display: flex; flex-direction: column; gap: 8px; }
  .host { display: flex; align-items: center; gap: 12px; padding: 10px 10px 10px 14px; }
  .host .who { flex: 1; }
  .host .kernel { max-width: 45%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .globe { display: grid; place-items: center; color: var(--muted); }
  .icon-button.open { color: var(--fg); background: var(--hover-strong); }
  .empty { padding: 18px; text-align: center; }
  .scrim { position: fixed; inset: 0; z-index: 19; background: none; border: none; cursor: default; }
  .scrim:hover:not(:disabled) { background: none; border: none; }
  .popover { position: absolute; top: calc(100% - 4px); right: 4px; z-index: 20; width: 250px; padding: 12px 14px; background: var(--popover); border: 1px solid var(--border-strong); border-radius: 10px; box-shadow: 0 10px 30px #0009; display: flex; flex-direction: column; gap: 10px; }
  dl { display: grid; grid-template-columns: auto 1fr; gap: 6px 14px; margin: 0; font-size: 0.923rem; }
  dt { color: var(--muted); }
  dd { margin: 0; text-align: right; overflow-wrap: anywhere; }
  .danger { align-self: flex-end; color: var(--error); }
  .end { display: flex; justify-content: flex-end; gap: 8px; margin-top: 12px; }
  .foot { display: flex; align-items: flex-start; gap: 6px; margin-top: 14px; color: var(--muted); font-size: 0.846rem; }
  .foot :global(svg) { flex: none; margin-top: 2px; }
  .backdrop { position: fixed; inset: 0; z-index: 30; display: grid; place-items: start center; padding-top: calc(var(--titlebar-h) + 40px); background: #0006; }
  .sheet { width: min(380px, 90vw); display: flex; flex-direction: column; gap: 6px; padding: 18px 20px 16px; background: var(--popover); border: 1px solid var(--border-strong); border-radius: 12px; box-shadow: 0 20px 50px #000b; }
  .sheet > small { margin-bottom: 8px; }
  .field { display: grid; grid-template-columns: 70px 1fr; align-items: center; gap: 10px; }
  .field span { text-align: right; color: var(--muted); }
  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 10px; }
  .keys { display: flex; gap: 6px; align-items: center; }
  .combo { min-width: 120px; justify-content: center; font-family: var(--mono); font-size: 0.885rem; }
  .combo.recording { border-color: var(--accent); color: var(--accent); }
  .reset { color: var(--muted); border-color: transparent; font-size: 0.885rem; }
  .mono { font-family: var(--mono); font-size: 0.885rem; }
  .muted { color: var(--muted); }
  p { margin: 8px 0; font-size: 0.923rem; }
  .error { color: var(--error); }
  .row input[type='number'] { width: 80px; text-align: right; background: var(--bg); border: 1px solid var(--border); border-radius: 5px; padding: 5px 9px; outline: none; }
  @media (max-width: 760px) {
    nav { width: 160px; padding: 10px 6px; }
    main { padding: 12px 18px 48px; }
    .combo { min-width: 90px; }
  }
</style>

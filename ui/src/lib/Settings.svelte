<script lang="ts">
  import { inTauri, type Api, type Host, type HostStatus, type TailscaleStatus } from './ipc'
  import { account, platform, settings, system } from './state.svelte'
  import Icon from './Icon.svelte'
  import { BACK, CODE, FORWARD, GLOBE, INFO, KEY, KEYBOARD, PALETTE, PERSON, SEARCH, SERVER } from './icons'
  import FontPicker from './FontPicker.svelte'
  import Preview from './settings/Preview.svelte'
  import ThemeGrid from './settings/ThemeGrid.svelte'
  import Segmented from './settings/Segmented.svelte'
  import Slider from './settings/Slider.svelte'
  import Stepper from './settings/Stepper.svelte'
  import Swatches from './settings/Swatches.svelte'
  import { actions, activeTheme, combo, defaults, label, type Action } from './settings'
  import { highlightPython } from './highlight'
  import { onMount } from 'svelte'

  let { api, start = 'server' }: { api: Api; start?: string } = $props()
  const sections = [
    ['server', 'Server', 'When Tailscale is on, your other devices on the same Tailscale network can open notebooks and run kernels on this Mac.', SERVER, '', 'tailscale device name dns start stop restart'],
    ['appearance', 'Appearance', 'Theme, accent color, window, layout and fonts.', PALETTE, '#bf5af2', 'light dark auto theme accent color translucent window opacity padding corner radius font size interface markdown code'],
    ['editor', 'Editor', 'How code and markdown cells behave while you edit.', CODE, '#0a84ff', 'cursor blink line height tab size ligatures line numbers word wrap lint type diagnostics double click agents'],
    ['keyboard', 'Keyboard', 'Click a shortcut, then press the new key combination.', KEYBOARD, '#8e8e93', `shortcuts ${actions.map((a) => a[1]).join(' ')}`],
    ['account', 'Account', 'Your Google account for Colab runtimes.', PERSON, '#30d158', 'google sign in oauth client id secret'],
    ['hosts', 'Remote Hosts', 'Machines on your Tailscale network that run colab-server with Tailscale turned on. The list is shared with colab-server hosts.', GLOBE, '#5e5ce6', 'add remove tailscale'],
    ['secrets', 'Secrets', 'Saved in your macOS Keychain. Every notebook, local or Colab, gets them as environment variables and through google.colab.userdata.get.', KEY, '#ff9f0a', 'keychain environment variables'],
    ['about', 'About', 'Version information.', INFO, '#636366', 'version'],
  ] as const
  type Section = (typeof sections)[number][0]
  const first = sections.find((s) => s[0] === start)?.[0] ?? 'server'
  let trail = $state<Section[]>([first])
  let at = $state(0)
  const section = $derived(trail[at])
  const current = $derived(sections.find((s) => s[0] === section)!)
  let query = $state('')
  const shown = $derived(sections.filter((s) => `${s[1]} ${s[5]}`.toLowerCase().includes(query.trim().toLowerCase())))
  const traffic = inTauri && platform.mac
  const sample = highlightPython('import numpy as np\n\ndef scale(x: np.ndarray, k=2.5) -> np.ndarray:\n    # keep values at or above zero\n    return np.where(x >= 0, x * k, 0)')
  const fontRows = [['Interface', 'interfaceFont', 'interfaceSize', 10, 20], ['Markdown', 'markdownFont', 'markdownSize', 10, 28], ['Code', 'codeFont', 'codeSize', 8, 32]] as const
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
      if (e.key === 'Escape' && adding) (adding = false), (hostError = '')
      else if (e.key === 'Escape') info = null
      return
    }
    e.preventDefault()
    e.stopPropagation()
    const c = e.key === 'Escape' ? null : combo(e)
    if (c) settings.keys[recording] = c
    if (c || e.key === 'Escape') recording = null
  }
</script>


<svelte:window onkeydowncapture={key} />

<div class="settings" class:traffic>
  <nav aria-label="Settings sections">
    <div class="drag" data-tauri-drag-region></div>
    <label class="search"><Icon d={SEARCH} size={13} /><input type="search" bind:value={query} placeholder="Search Settings" aria-label="Search settings" /></label>
    {#each shown as [id, name, , icon, color] (id)}
      <button class:active={section === id} class:server={id === 'server'} aria-current={section === id ? 'page' : undefined} onclick={() => go(id)}>
        {#if id === 'server'}
          <span class="avatar"><Icon d={icon} size={16} /></span>
          <span class="who">Server<small>{server || 'Checking...'}</small></span>
        {:else}
          <span class="badge" style:background={color}><Icon d={icon} size={13} /></span>{name}
        {/if}
      </button>
    {:else}
      <p class="none">No results</p>
    {/each}
  </nav>
  <main>
    <header class="head" data-tauri-drag-region>
      <div class="pill glass-pill">
        <button onclick={() => at--} disabled={at === 0} aria-label="Back"><Icon d={BACK} size={15} /></button>
        <button onclick={() => at++} disabled={at === trail.length - 1} aria-label="Forward"><Icon d={FORWARD} size={15} /></button>
      </div>
      <h1 data-tauri-drag-region>{current[1]}</h1>
    </header>
    <div class="body">
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
        <Preview {sample} />
        <div class="card">
          <div class="row"><span>Appearance</span><Segmented bind:value={settings.appearance} options={[['light', 'Light'], ['dark', 'Dark'], ['auto', 'Auto']]} label="Appearance" /></div>
          <div class="row"><span>Accent color</span><Swatches bind:value={settings.accent} fallback={activeTheme(settings, system.dark).vars['--accent']} /></div>
        </div>
        <h3>Theme</h3>
        <div class="card padded"><ThemeGrid {sample} /></div>
        {#if platform.mac}
          <h3>Window</h3>
          <div class="card">
            <label class="row"><span>Translucent window<small>Shows the desktop through the sidebar and title bar</small></span><input type="checkbox" bind:checked={settings.glass} /></label>
            {#if settings.glass}<div class="row"><span>Content opacity</span><Slider bind:value={settings.opacity} min={60} max={100} label="Content opacity" unit="%" /></div>{/if}
          </div>
        {/if}
        <h3>Layout</h3>
        <div class="card">
          <div class="row"><span>Notebook padding</span><Slider bind:value={settings.padding} min={0} max={48} step={4} label="Notebook padding" unit=" px" /></div>
          <div class="row"><span>Corner radius</span><Slider bind:value={settings.radius} min={0} max={16} label="Corner radius" unit=" px" /></div>
        </div>
        <h3>Fonts</h3>
        <div class="card">
          {#each fontRows as [name, font, size, min, max] (name)}
            <div class="row"><span>{name}</span><span class="controls"><FontPicker bind:value={settings[font]} fonts={font === 'codeFont' ? mono : fonts} label="{name} font" /><Stepper bind:value={settings[size]} {min} {max} label="{name} font size" /></span></div>
          {/each}
        </div>
      {:else if section === 'editor'}
        <h3>Cursor</h3>
        <div class="card">
          <div class="row"><span>Style</span><Segmented bind:value={settings.cursorStyle} options={[['bar', 'Bar'], ['block', 'Block'], ['underline', 'Underline']]} label="Cursor style" /></div>
          <label class="row"><span>Blink</span><input type="checkbox" bind:checked={settings.cursorBlink} /></label>
        </div>
        <h3>Code</h3>
        <div class="card">
          <div class="row"><span>Line height</span><Stepper bind:value={settings.lineHeight} min={1} max={3} step={0.1} label="Line height" /></div>
          <div class="row"><span>Tab size</span><Stepper bind:value={settings.tabSize} min={1} max={8} label="Tab size" /></div>
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
        <h3>Agents</h3>
        <div class="card">
          <label class="row"><span>Open notebooks that agents use<small>Shows a window when an agent opens a notebook, and closes it when the agent is done</small></span><input type="checkbox" checked={agentNotebooks} onchange={(e) => api.setOpenAgentNotebooks((agentNotebooks = e.currentTarget.checked)).catch((err) => (agentError = String(err)))} /></label>
          {#if agentError}<p class="error">{agentError}</p>{/if}
        </div>
      {:else if section === 'keyboard'}
        <h3>Shortcuts</h3>
        <div class="card">
          {#each actions as [id, name] (id)}
            <div class="row">
              <span>{name}</span>
              <div class="keys">
                {#if settings.keys[id] !== defaults.keys[id]}<button class="reset" onclick={() => (settings.keys[id] = defaults.keys[id])} title="Reset to {label(defaults.keys[id])}">Reset</button>{/if}
                <button class="combo outline" class:recording={recording === id} onclick={() => (recording = recording === id ? null : id)} aria-label="{name} shortcut">{recording === id ? 'Press keys...' : label(settings.keys[id])}</button>
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
              <button class="icon-button" class:open={info === h.label} onclick={() => (info = info === h.label ? null : h.label)} aria-label="Details for {h.label}" aria-expanded={info === h.label}><Icon d={INFO} size={16} /></button>
              {#if info === h.label}
                <button class="scrim" onclick={() => (info = null)} aria-label="Close details"></button>
                <div class="popover pop" role="dialog" aria-label="{h.label} details">
                  <dl>
                    <dt>Address</dt><dd class="mono">{h.address}</dd>
                    <dt>Path</dt><dd>{s?.path === 'direct' ? 'Direct' : s?.path === 'relay' ? 'Relay' : 'None'}</dd>
                    <dt>Latency</dt><dd>{s?.latency_ms != null ? `${s.latency_ms} ms` : 'Unknown'}</dd>
                  </dl>
                  <button class="danger" onclick={() => editHosts(api.removeHost(h.label))}>Remove</button>
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
              <button class="outline" onclick={() => (s.shown = !s.shown)}>{s.shown ? 'Hide' : 'Show'}</button>
              <button class="danger" onclick={() => deleteSecret(i)}>Delete</button>
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
    </div>
  </main>
</div>

{#if adding}
  <div class="backdrop" role="dialog" aria-modal="true" aria-label="Add Remote Host">
    <form class="sheet pop" onsubmit={(e) => (e.preventDefault(), editHosts(api.addHost(draft.label.trim(), draft.address.trim())))}>
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
  .settings { flex: 1; min-height: 0; display: flex; --card: color-mix(in srgb, var(--fg) 5%, var(--surface)); }
  nav { width: 224px; flex: none; display: flex; flex-direction: column; gap: 1px; padding: 0 10px 12px; box-sizing: border-box; background: var(--chrome); overflow-y: auto; -webkit-user-select: none; user-select: none; }
  .drag { flex: none; height: 12px; }
  .traffic .drag { height: var(--titlebar-h); }
  .search { display: flex; align-items: center; gap: 6px; min-height: 30px; padding: 0 10px; margin: 0 0 12px; border-radius: 999px; background: var(--glass-hover); box-shadow: inset 0 0 0 0.5px var(--hairline); color: var(--muted); }
  .search:focus-within { box-shadow: var(--ring); }
  .search input { flex: 1; min-width: 0; border: none; background: none; padding: 4px 0; outline: none; }
  .search input::-webkit-search-cancel-button { cursor: pointer; }
  nav button { justify-content: flex-start; gap: 9px; min-height: 32px; padding: 0 8px; border-radius: 10px; font-weight: 500; }
  nav button.active, nav button.active:hover:not(:disabled) { background: var(--glass-strong); }
  nav button.server { min-height: 50px; margin-bottom: 12px; }
  .none { padding: 0 8px; color: var(--muted); }
  nav small, .search, .search input::placeholder { color: color-mix(in srgb, var(--fg) 62%, transparent); }
  .badge { display: grid; place-items: center; width: 22px; height: 22px; border-radius: 6px; color: white; flex: none; box-shadow: inset 0 0.5px 0 #ffffff59, 0 0.5px 1.5px #0000004d; }
  .avatar { display: grid; place-items: center; width: 32px; height: 32px; border-radius: 50%; color: white; flex: none; background: linear-gradient(#8e8e93, #48484a); box-shadow: inset 0 0.5px 0 #ffffff59; }
  .avatar.big { width: 52px; height: 52px; }
  .who { display: flex; flex-direction: column; min-width: 0; text-align: left; line-height: 1.3; }
  .who small, .who strong { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  main { flex: 1; min-width: 0; display: flex; flex-direction: column; background: var(--surface); }
  .head { flex: none; display: flex; align-items: center; gap: 14px; height: 56px; padding: 6px 24px 0; box-sizing: border-box; -webkit-user-select: none; user-select: none; }
  .body { flex: 1; min-height: 0; overflow-y: auto; padding: 8px 24px 48px; }
  .body > * { max-width: 680px; box-sizing: border-box; margin-inline: auto; }
  .pill { position: relative; display: flex; flex: none; }
  .pill button { width: 38px; height: 34px; padding: 0; justify-content: center; border: none; color: var(--fg); }
  .pill button:disabled { color: var(--faint); }
  .pill button + button::before { content: ''; position: absolute; left: 50%; top: 9px; bottom: 9px; width: 0.5px; background: var(--hairline); }
  h1 { flex: 1; min-width: 0; margin: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 1.154rem; font-weight: 600; }
  h3 { margin: 22px 10px 7px; font-size: 0.923rem; font-weight: 600; }
  .body > :first-child { margin-top: 8px; }
  .card { position: relative; display: flow-root; padding: 0 14px; border-radius: 12px; background: var(--card); box-shadow: inset 0 0 0 0.5px color-mix(in srgb, var(--fg) 5%, transparent); }
  .card.padded { padding: 14px; }
  :global(.preview) + .card { margin-top: 16px; }
  .hero { display: flex; align-items: center; gap: 12px; padding: 14px; }
  .hero strong { font-size: 1.077rem; font-weight: 600; }
  .hero .who { flex: 1; }
  .state { display: flex; align-items: center; gap: 6px; color: var(--muted); }
  .state i { width: 8px; height: 8px; border-radius: 50%; background: var(--warn); }
  .state i.ok { background: var(--ok); }
  .state i.off { background: var(--muted); }
  .row { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 8px 16px; padding: 10px 0; min-height: 26px; }
  .row + .row { border-top: 0.5px solid var(--hairline); }
  .row > span { display: flex; flex-direction: column; gap: 2px; min-width: 0; flex: 1 1 120px; }
  .row > span:first-child:not(.muted) { font-weight: 600; }
  .row > span.controls { flex: 1 1 0; flex-direction: row; align-items: center; justify-content: flex-end; gap: 8px; font-weight: 400; }
  small { color: var(--muted); font-size: 0.846rem; font-weight: 400; }
  .value { color: var(--muted); }
  .row > span:is(.value, .mono) { flex: 0 1 auto; text-align: right; }
  .row > input:not([type='checkbox']) { width: min(240px, 50%); box-sizing: border-box; }
  .row input[type='checkbox'] { appearance: none; position: relative; flex: none; width: 38px; height: 22px; margin: 0 0 0 auto; border-radius: 11px; background: color-mix(in srgb, var(--fg) 16%, transparent); box-shadow: inset 0 0 0 0.5px var(--hairline); cursor: pointer; }
  .row input[type='checkbox']::before { content: ''; position: absolute; top: 2px; left: 2px; width: 18px; height: 18px; border-radius: 50%; background: white; box-shadow: 0 1px 3px #00000059, 0 0 0 0.5px #0000001a; }
  .row input[type='checkbox']:checked { background: var(--accent); }
  .row input[type='checkbox']:checked::before { transform: translateX(16px); }
  .row input[type='checkbox']:focus-visible { outline: none; box-shadow: var(--ring); }
  .row input:disabled { opacity: 0.5; cursor: default; }
  .row button, .end button, .actions button { min-height: 28px; }
  .row.secret { flex-wrap: nowrap; justify-content: flex-start; gap: 8px; }
  .row.secret input { flex: 1; width: auto; min-width: 0; }
  .hosts { display: flex; flex-direction: column; gap: 8px; }
  .host { display: flex; align-items: center; gap: 12px; padding: 10px 10px 10px 14px; }
  .host .who { flex: 1; }
  .host .kernel { max-width: 45%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .globe { display: grid; place-items: center; color: var(--muted); }
  .icon-button.open { color: var(--fg); background: var(--glass-hover); }
  .empty { padding: 18px; text-align: center; }
  .scrim { position: fixed; inset: 0; z-index: 19; background: none; border: none; cursor: default; }
  .scrim:hover:not(:disabled) { background: none; }
  .popover { position: absolute; top: calc(100% - 4px); right: 4px; z-index: 20; width: 248px; padding: 14px; box-sizing: border-box; display: flex; flex-direction: column; gap: 12px; }
  dl { display: grid; grid-template-columns: auto 1fr; gap: 4px 12px; margin: 0; font-size: 0.923rem; }
  dt { color: var(--muted); }
  dd { margin: 0; text-align: right; overflow-wrap: anywhere; }
  .popover .danger { align-self: flex-end; }
  .end { display: flex; justify-content: flex-end; gap: 8px; margin-top: 12px; }
  .foot { display: flex; align-items: flex-start; gap: 6px; margin-top: 18px; padding-inline: 10px; color: var(--muted); font-size: 0.846rem; }
  .foot :global(svg) { flex: none; margin-top: 2px; }
  .backdrop { position: fixed; inset: 0; z-index: 30; display: grid; place-items: start center; padding-top: calc(var(--titlebar-h) + 40px); background: light-dark(#00000026, #00000066); }
  .sheet { width: min(380px, 90vw); display: flex; flex-direction: column; gap: 8px; padding: 22px; border-radius: 20px; }
  .sheet > strong { font-weight: 600; }
  .sheet > small { margin-bottom: 8px; }
  .field { display: grid; grid-template-columns: 72px 1fr; align-items: center; gap: 12px; }
  .field span { text-align: right; color: var(--muted); }
  .actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 12px; }
  .keys { display: flex; gap: 8px; align-items: center; margin-left: auto; }
  .combo { min-width: 120px; justify-content: center; font-family: var(--mono); font-size: 0.885rem; }
  .combo.recording { color: var(--accent); box-shadow: var(--glass-rim), var(--ring); }
  .reset { color: var(--muted); font-size: 0.885rem; }
  .mono { font-family: var(--mono); font-size: 0.885rem; }
  .muted { color: var(--muted); }
  p { margin: 10px 0; font-size: 0.923rem; }
  .error { color: var(--error); }
  @media (prefers-reduced-motion: no-preference) {
    .row input[type='checkbox'] { transition: background-color 150ms; }
    .row input[type='checkbox']::before { transition: transform 150ms; }
  }
  @media (max-width: 760px) {
    nav { width: 188px; padding-inline: 8px; }
    .head { padding-inline: 16px; }
    .body { padding-inline: 16px; }
    .combo { min-width: 88px; }
  }
</style>

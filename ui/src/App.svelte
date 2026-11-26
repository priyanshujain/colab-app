<script lang="ts">
  import { onMount } from 'svelte'
  import type { OpenedNotebook } from './bindings/OpenedNotebook'
  import CloneDialog from './lib/CloneDialog.svelte'
  import Editor from './lib/Editor.svelte'
  import Icon from './lib/Icon.svelte'
  import { GEAR } from './lib/icons'
  import { api as ready, inTauri, type Api } from './lib/ipc'
  import Notebook from './lib/Notebook.svelte'
  import Settings from './lib/Settings.svelte'
  import RemotePicker from './lib/RemotePicker.svelte'
  import { account, alerts, remote } from './lib/state.svelte'
  import Titlebar from './lib/Titlebar.svelte'

  let api = $state.raw<Api>()
  let opened = $state<OpenedNotebook | null>(null)
  let recentKey = 'recent'
  let recent = $state<string[]>([])
  let browsing = $state(false)
  let outdated = $state<string | null>(null)
  let error = $state('')
  let cloning = $state(false)
  const params = new URLSearchParams(location.search)
  const settingsView = params.get('view') === 'settings'
  let section = $state(params.get('section') || undefined)
  let sectionPage = $state(0)
  let viewer = $state<{ path: string; text: string; line: number; character: number } | null>(null)

  async function load(path: string) {
    try {
      const result = await api!.openNotebook(path)
      if (!result) return
      opened = result
      recent = [path, ...recent.filter((p) => p !== path)].slice(0, 8)
      localStorage.setItem(recentKey, JSON.stringify(recent))
      error = ''
    } catch (e) {
      error = String(e)
    }
  }

  function renamed(old: string, path: string) {
    recent = recent.filter((p) => p !== old)
    load(path)
  }

  const pick = async () => {
    if (remote.host) return (browsing = true)
    const p = await api!.pickNotebook()
    if (p) load(p)
  }

  function menu(id: string) {
    if (id === 'open') pick()
    else if (id === 'clone') cloning = !remote.host
    else if (id === 'settings') api?.openSettings()
    else if (id === 'server') api?.openSettings('server')
    else if (id.startsWith('open-recent:')) load(id.slice('open-recent:'.length))
  }

  async function openFile(path: string, line: number, character: number) {
    try {
      viewer = { path, text: await api!.readFile(path), line, character }
    } catch (e) {
      error = String(e)
    }
  }

  onMount(() => {
    const key = (e: KeyboardEvent) => {
      if (!inTauri && !settingsView && e.metaKey && e.key === 'o') (e.preventDefault(), menu('open'))
      if (!inTauri && !settingsView && e.metaKey && e.key === ',') (e.preventDefault(), menu('settings'))
      if (e.key === 'Escape' && viewer) viewer = null
      if ((e.metaKey || e.ctrlKey) && e.code === 'KeyR') e.preventDefault()
    }
    const click = (e: MouseEvent) => {
      const a = (e.target as Element).closest?.('a[href]') as HTMLAnchorElement | null
      if (!a || !/^https?:/.test(a.href) || new URL(a.href).origin === location.origin) return
      e.preventDefault()
      api?.openUrl(a.href).catch((err) => (error = String(err)))
    }
    window.addEventListener('keydown', key)
    window.addEventListener('click', click, true)
    ready.then(async (a) => {
      if (settingsView) {
        api = a
        a.onMenu((id) => id && ((section = id), sectionPage++))
        a.onAuthState((s) => { account.auth = s })
        account.auth = await a.authState().catch(() => account.auth)
        return
      }
      remote.host = await a.windowHost()
      recentKey = remote.host ? `recent:${remote.host}` : 'recent'
      recent = JSON.parse(localStorage.getItem(recentKey) ?? '[]')
      api = a
      a.onMenu(menu)
      a.onServerOutdated((v) => (outdated = v))
      a.onAuthState((s) => { account.auth = s })
      account.auth = await a.authState().catch(() => account.auth)
      const startup = await a.windowStartup()
      if (startup !== null) return startup && menu(startup)
      const initial = params.get('path') ?? recent[0] ?? (inTauri ? null : 'sample.ipynb')
      if (initial) load(initial)
    })
    return () => (window.removeEventListener('keydown', key), window.removeEventListener('click', click, true))
  })
</script>

{#if settingsView}
  {#if api}{#key sectionPage}<Settings {api} start={section} />{/key}{/if}
{:else}
  {#if !opened}
    <Titlebar>
      {#snippet center()}<span class="title" data-tauri-drag-region>Colab</span>{/snippet}
      {#snippet trail()}<button class="icon-button" onclick={() => api?.openSettings()} title="Settings" aria-label="Settings"><Icon d={GEAR} /></button>{/snippet}
    </Titlebar>
  {/if}

  {#if outdated}<div class="notice" role="status">{outdated}</div>{/if}

  {#if error || alerts.error}<div class="banner" role="alert">{error || alerts.error}<button onclick={() => ((error = ''), (alerts.error = ''))} aria-label="Dismiss">&times;</button></div>{/if}

  {#if api && opened}
    <div class="page">
      {#key opened.path}
        <Notebook {api} path={opened.path} initial={opened.notebook.cells} metadata={opened.notebook.metadata} onopenfile={openFile} onrenamed={(p) => renamed(opened!.path, p)} />
      {/key}
    </div>
  {:else if api}
    <main class="welcome">
      <h1>Open a notebook</h1>
      <div class="actions">
        <button class="primary" onclick={() => menu('open')}>Open notebook <kbd>&#8984;O</kbd></button>
        {#if !remote.host}<button class="outline" onclick={() => (cloning = true)}>Clone from Colab link</button>{/if}
      </div>
      {#if recent.length}
        <h2>Recent</h2>
        <ul>
          {#each recent as p (p)}<li><button onclick={() => load(p)}><span>{p.split('/').at(-1)}</span><span class="dir">{p}</span></button></li>{/each}
        </ul>
      {/if}
    </main>
  {/if}

  {#if viewer}
    <aside class="viewer">
      <div class="viewer-head">
        <span class="dir" title={viewer.path}>{viewer.path}</span>
        <span class="badge">read only</span>
        <button onclick={() => (viewer = null)} aria-label="Close">&times;</button>
      </div>
      {#key viewer}
        <div class="viewer-body"><Editor source={viewer.text} language="python" readonly reveal={{ line: viewer.line, character: viewer.character }} /></div>
      {/key}
    </aside>
  {/if}

  {#if api && browsing && remote.host}
    <RemotePicker {api} host={remote.host} start={opened?.path.replace(/\/[^/]*$/, '') ?? ''} onclose={() => (browsing = false)} onopen={(p) => ((browsing = false), load(p))} />
  {/if}

  {#if api && cloning}
    <CloneDialog {api} onclose={() => (cloning = false)} onopen={(p) => ((cloning = false), load(p))} />
  {/if}
{/if}

<style>
  .title { font-weight: 500; color: var(--fg); }
  .page { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .dir { color: var(--faint); font-size: 0.923rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .banner { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 6px 14px; background: linear-gradient(color-mix(in srgb, var(--error) 9.4%, transparent), color-mix(in srgb, var(--error) 9.4%, transparent)), var(--surface); color: var(--error); border-bottom: 1px solid color-mix(in srgb, var(--error) 25%, transparent); font-size: 0.962rem; }
  .notice { padding: 6px 14px; background: linear-gradient(var(--hover-strong), var(--hover-strong)), var(--surface); color: var(--muted); border-bottom: 0.5px solid var(--hairline); font-size: 0.962rem; }
  .welcome { flex: 1; background: var(--surface); box-shadow: inset 0 0.5px 0 var(--hairline); display: flex; flex-direction: column; align-items: center; padding-top: 18vh; gap: 10px; }
  .welcome h1 { font-size: 1.538rem; font-weight: 600; margin: 0 0 8px; }
  .welcome h2 { align-self: center; width: min(460px, 90vw); padding-left: 24px; box-sizing: border-box; font-size: 0.923rem; color: var(--muted); margin: 28px 0 2px; font-weight: 600; }
  .actions { display: flex; gap: 10px; }
  .actions button { min-height: 32px; padding-inline: 16px; }
  .primary kbd { color: inherit; border-color: color-mix(in srgb, var(--button-fg) 25%, transparent); }
  ul { list-style: none; padding: 6px; margin: 0; width: min(460px, 90vw); box-sizing: border-box; border-radius: 16px; background: color-mix(in srgb, var(--fg) 4%, transparent); }
  li button span { overflow: hidden; text-overflow: ellipsis; }
  li button { width: 100%; justify-content: space-between; gap: 16px; padding: 7px 12px; border-radius: 10px; }
  li button span:first-child { font-weight: 500; }
  .viewer { position: fixed; top: var(--titlebar-h); right: 0; bottom: 0; width: min(720px, 85vw); background: var(--editor-bg); border-left: 1px solid var(--border-strong); box-shadow: -16px 0 40px light-dark(#0000001f, #00000070); display: flex; flex-direction: column; z-index: 10; }
  .viewer-head { display: flex; align-items: center; gap: 8px; padding: 6px 10px 6px 14px; border-bottom: 1px solid var(--border); }
  .viewer-head .dir { flex: 1; font: 0.923rem var(--mono); color: var(--muted); }
  .badge { font-size: 0.846rem; color: var(--muted); background: var(--glass-hover); border-radius: 999px; padding: 1px 9px; }
  .viewer-body { flex: 1; overflow: auto; }
</style>

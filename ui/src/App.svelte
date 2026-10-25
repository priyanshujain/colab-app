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
  import { account, alerts } from './lib/state.svelte'
  import Titlebar from './lib/Titlebar.svelte'

  let api = $state.raw<Api>()
  let opened = $state<OpenedNotebook | null>(null)
  let recent = $state<string[]>(JSON.parse(localStorage.getItem('recent') ?? '[]'))
  let error = $state('')
  let cloning = $state(false)
  let settingsOpen = $state(false)
  let viewer = $state<{ path: string; text: string; line: number; character: number } | null>(null)

  async function load(path: string) {
    try {
      opened = await api!.openNotebook(path)
      recent = [path, ...recent.filter((p) => p !== path)].slice(0, 8)
      localStorage.setItem('recent', JSON.stringify(recent))
      settingsOpen = false
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
    const p = await api!.pickNotebook()
    if (p) load(p)
  }

  function menu(id: string) {
    if (id === 'open') pick()
    else if (id === 'clone') cloning = true
    else if (id === 'settings') settingsOpen = true
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
      if (!inTauri && e.metaKey && e.key === 'o') (e.preventDefault(), menu('open'))
      if (!inTauri && e.metaKey && e.key === ',') (e.preventDefault(), menu('settings'))
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
      api = a
      a.onMenu(menu)
      a.onAgentOpen((e) => load(e.path))
      a.onAuthState((s) => { account.auth = s })
      account.auth = await a.authState().catch(() => account.auth)
      const initial = new URLSearchParams(location.search).get('path') ?? recent[0] ?? (inTauri ? null : 'sample.ipynb')
      if (initial) load(initial)
    })
    return () => (window.removeEventListener('keydown', key), window.removeEventListener('click', click, true))
  })
</script>

{#if api && settingsOpen}
  <Settings {api} onback={() => (settingsOpen = false)} />
{:else if !opened}
  <Titlebar>
    {#snippet center()}<span class="title" data-tauri-drag-region>Colab</span>{/snippet}
    {#snippet trail()}<button class="icon-button" onclick={() => (settingsOpen = true)} title="Settings" aria-label="Settings"><Icon d={GEAR} /></button>{/snippet}
  </Titlebar>
{/if}

{#if error || alerts.error}<div class="banner" role="alert">{error || alerts.error}<button onclick={() => ((error = ''), (alerts.error = ''))} aria-label="Dismiss">&times;</button></div>{/if}

{#if api && opened}
  <div class="page" class:hidden={settingsOpen}>
    {#key opened.path}
      <Notebook {api} path={opened.path} initial={opened.notebook.cells} metadata={opened.notebook.metadata} onopenfile={openFile} onsettings={() => (settingsOpen = true)} onrenamed={(p) => renamed(opened!.path, p)} />
    {/key}
  </div>
{:else if api && !settingsOpen}
  <main class="welcome">
    <h1>Open a notebook</h1>
    <div class="actions">
      <button class="primary" onclick={() => menu('open')}>Open notebook <kbd>&#8984;O</kbd></button>
      <button class="outline" onclick={() => (cloning = true)}>Clone from Colab link</button>
    </div>
    {#if recent.length}
      <h2>Recent</h2>
      <ul>
        {#each recent as p (p)}<li><button onclick={() => load(p)}><span>{p.split('/').at(-1)}</span><span class="dir">{p}</span></button></li>{/each}
      </ul>
    {/if}
  </main>
{/if}

{#if viewer && !settingsOpen}
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

{#if api && cloning}
  <CloneDialog {api} onclose={() => (cloning = false)} onopen={(p) => ((cloning = false), load(p))} />
{/if}

<style>
  .title { font-weight: 500; color: var(--fg); }
  .page { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .hidden { display: none; }
  .dir { color: var(--faint); font-size: 0.923rem; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .banner { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 6px 14px; background: color-mix(in srgb, var(--error) 9.4%, transparent); color: var(--error); border-bottom: 1px solid color-mix(in srgb, var(--error) 25%, transparent); font-size: 0.962rem; }
  .welcome { flex: 1; display: flex; flex-direction: column; align-items: center; padding-top: 18vh; gap: 10px; }
  .welcome h1 { font-size: 1.538rem; font-weight: 600; margin: 0 0 8px; }
  .welcome h2 { font-size: 0.923rem; text-transform: uppercase; letter-spacing: 0.06em; color: var(--muted); margin: 28px 0 4px; font-weight: 600; }
  .actions { display: flex; gap: 8px; }
  .primary kbd { color: inherit; border-color: color-mix(in srgb, var(--button-fg) 25%, transparent); }
  ul { list-style: none; padding: 0; margin: 0; width: min(460px, 90vw); }
  li button span { overflow: hidden; text-overflow: ellipsis; }
  li button { width: 100%; justify-content: space-between; padding: 6px 10px; }
  .viewer { position: fixed; top: var(--titlebar-h); right: 0; bottom: 0; width: min(720px, 85vw); background: var(--editor-bg); border-left: 1px solid var(--border-strong); box-shadow: -16px 0 40px #0007; display: flex; flex-direction: column; z-index: 10; }
  .viewer-head { display: flex; align-items: center; gap: 8px; padding: 6px 10px 6px 14px; border-bottom: 1px solid var(--border); }
  .viewer-head .dir { flex: 1; font: 0.923rem var(--mono); color: var(--muted); }
  .badge { font-size: 0.846rem; color: var(--muted); border: 1px solid var(--border); border-radius: 10px; padding: 0 8px; }
  .viewer-body { flex: 1; overflow: auto; }
</style>

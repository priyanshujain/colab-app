<script lang="ts">
  import type { Api } from './ipc'

  let { api, onclose, onopen }: { api: Api; onclose: () => void; onopen: (path: string) => void } = $props()
  let url = $state('')
  let busy = $state('')
  let error = $state('')

  async function clone() {
    error = ''
    try {
      busy = 'Checking link...'
      const name = await api.checkLink(url.trim())
      busy = 'Choosing location...'
      const dir = localStorage.getItem('cloneDir')
      const dest = await api.pickSave(dir ? `${dir}/${name}` : name)
      if (!dest) return
      busy = 'Cloning...'
      const path = await api.cloneLink(url.trim(), dest)
      localStorage.setItem('cloneDir', dest.replace(/\/[^/]*$/, ''))
      onopen(path)
    } catch (e) {
      error = String(e).replace(/^Error: /, '')
    } finally {
      busy = ''
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={(e) => e.target === e.currentTarget && onclose()} onkeydown={(e) => e.key === 'Escape' && onclose()}>
  <form class="dialog" onsubmit={(e) => (e.preventDefault(), clone())}>
    <h2>Clone from Colab</h2>
    <label for="clone-url">Colab link</label>
    <!-- svelte-ignore a11y_autofocus -->
    <input id="clone-url" bind:value={url} placeholder="https://colab.research.google.com/drive/..." autofocus />
    <p class="hint">Drive notebooks must be shared as Anyone with the link. You choose where to save after the link is checked.</p>
    {#if error}<p class="error">{error}</p>{/if}
    <div class="actions">
      <button type="button" onclick={onclose}>Cancel</button>
      <button class="primary" disabled={!!busy || !url.trim()}>{busy || 'Clone and open'}</button>
    </div>
  </form>
</div>

<style>
  .backdrop { position: fixed; inset: 0; background: #0009; display: grid; place-items: start center; padding-top: 14vh; z-index: 30; }
  .dialog { width: 480px; background: var(--popover); border: 1px solid var(--border-strong); border-radius: 10px; padding: 18px 20px; box-shadow: 0 20px 50px #000a; display: flex; flex-direction: column; gap: 6px; }
  h2 { margin: 0 0 8px; font-size: 1.154rem; font-weight: 600; }
  label { color: var(--muted); font-size: 0.923rem; margin-top: 4px; }
  input { font-family: var(--mono); font-size: 0.923rem; }
  .hint { color: var(--faint); margin: 2px 0 0; font-size: 0.885rem; }
  .error { color: var(--error); margin: 4px 0 0; font-size: 0.962rem; }
  .actions { display: flex; justify-content: flex-end; gap: 6px; margin-top: 12px; }
</style>

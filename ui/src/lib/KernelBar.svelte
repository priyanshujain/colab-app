<script lang="ts">
  import { onMount } from 'svelte'
  import type { ColabInfo } from '../bindings/ColabInfo'
  import type { KernelChoice } from '../bindings/KernelChoice'
  import type { KernelStatus } from '../bindings/KernelStatus'
  import type { KernelStatusEvent } from '../bindings/KernelStatusEvent'
  import type { Variant } from '../bindings/Variant'
  import type { Api } from './ipc'
  import { account, settings } from './state.svelte'

  let { api, path, wide, status = $bindable('disconnected') }: { api: Api; path: string; wide: boolean; status?: KernelStatus } = $props()

  let active = $state<KernelChoice | null>(null)
  let info = $state<ColabInfo | null>(null)
  let error = $state('')
  let kind = $state<'local' | 'colab'>('local')
  let runtime = $state('DEFAULT')
  let highRam = $state(false)
  let busy = $state(false)

  const runtimes = $derived([
    ['DEFAULT', 'CPU'],
    ...(info?.eligible_gpus ?? []).map((g) => [`GPU:${g}`, `GPU ${g}`]),
    ...(info?.eligible_tpus ?? []).map((t) => [`TPU:${t}`, `TPU ${t}`]),
  ])
  const live = $derived(status !== 'disconnected' && status !== 'dead')
  const onColab = $derived(active?.kind === 'colab' && live)
  const label = $derived(!active ? 'No kernel' : active.kind === 'local' ? 'Local' : `Colab ${active.accelerator ?? (active.variant === 'DEFAULT' ? 'CPU' : active.variant)}`)

  const attempt = (f: () => Promise<unknown>) => f().then(() => (error = ''), (e) => (error = String(e)))
  const refresh = () => attempt(async () => (info = await api.colabInfo()))

  function choose(k: 'local' | 'colab') {
    kind = k
    if (k === 'local') attempt(() => api.selectKernel(path, { kind: 'local' }))
    else if (!info) refresh()
  }

  async function connect() {
    busy = true
    await attempt(async () => {
      if (account.auth.state !== 'signed_in') account.auth = await api.signIn(settings.clientId.trim() || null, settings.clientSecret.trim() || null)
      if (account.auth.state !== 'signed_in') throw new Error('Sign in to Colab first (Settings)')
      const [variant, accelerator] = runtime.split(':') as [Variant, string?]
      await api.selectKernel(path, { kind: 'colab', variant, accelerator: accelerator ?? null, high_ram: highRam })
    })
    busy = false
  }

  onMount(() => {
    const apply = (e: KernelStatusEvent) => {
      if (e.path !== path) return
      status = e.status
      active = e.choice
      if (e.choice) kind = e.choice.kind
    }
    const sub = api.onKernelStatus(apply)
    sub.then(() => api.kernelState(path)).then(apply, () => {})
    return () => void sub.then((u) => u())
  })
</script>

<div class="bar">
  {#if error}<span class="error" title={error}>{error}</span>{/if}
  {#if wide && active}<span class="status">{label} <span class="state {status}">{status}</span></span>{/if}
  <select value={kind} onchange={(e) => choose(e.currentTarget.value as 'local' | 'colab')} aria-label="Kernel" title="Kernel">
    <option value="local">Local .venv</option>
    <option value="colab">Colab</option>
  </select>
  {#if kind === 'colab' && !onColab}
    <select bind:value={runtime} aria-label="Runtime type" title="Runtime type">
      {#each runtimes as [value, name] (value)}<option {value}>{name}</option>{/each}
    </select>
    <label class="check" title="High RAM"><input type="checkbox" bind:checked={highRam} />{wide ? 'High RAM' : 'RAM+'}</label>
    <button class="primary" onclick={connect} disabled={busy || status === 'connecting'}>{busy ? 'Connecting...' : 'Connect'}</button>
  {/if}
  {#if onColab}
    <button onclick={() => attempt(() => api.detach(path))} title="Disconnect but keep the runtime">Detach</button>
    <button onclick={() => attempt(() => api.release(path))} title="Release the runtime">Release</button>
  {/if}
</div>

<style>
  .bar { display: flex; align-items: center; gap: 6px; min-width: 0; }
  .status { color: var(--muted); white-space: nowrap; font-size: 0.923rem; margin: 0 4px; }
  .state { color: var(--faint); margin-left: 2px; }
  .state.dead { color: var(--error); }
  .check { display: inline-flex; align-items: center; gap: 4px; color: var(--muted); font-size: 0.923rem; white-space: nowrap; }
  .error { color: var(--error); max-width: 220px; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 0.923rem; }
</style>

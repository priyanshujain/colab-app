<script lang="ts">
  import { onMount } from 'svelte'
  import type { ColabInfo } from '../bindings/ColabInfo'
  import type { KernelChoice } from '../bindings/KernelChoice'
  import type { KernelStatus } from '../bindings/KernelStatus'
  import type { KernelStatusEvent } from '../bindings/KernelStatusEvent'
  import type { KernelType } from '../bindings/KernelType'
  import type { Machine } from '../bindings/Machine'
  import type { Variant } from '../bindings/Variant'
  import type { Api, HostStatus } from './ipc'
  import { account, settings } from './state.svelte'
  import { duration } from './timing'

  let { api, path, wide, status = $bindable('disconnected'), kind = $bindable('local'), started = $bindable(null), label = $bindable(''), detail = $bindable('') }: { api: Api; path: string; wide: boolean; status?: KernelStatus; kind?: string; started?: number | null; label?: string; detail?: string } = $props()

  let active = $state<KernelChoice | null>(null)
  let info = $state<ColabInfo | null>(null)
  let error = $state('')
  let place = $state('local')
  let runtime = $state('DEFAULT')
  let highRam = $state(false)
  let busy = $state(false)
  let picking = $state(false)
  let hosts = $state<HostStatus[]>([])
  let machine = $state<Machine | null>(null)

  const runtimes = $derived([
    ['DEFAULT', 'CPU'],
    ...(info?.eligible_gpus ?? []).map((g) => [`GPU:${g}`, `GPU ${g}`]),
    ...(info?.eligible_tpus ?? []).map((t) => [`TPU:${t}`, `TPU ${t}`]),
  ])
  const live = $derived(status !== 'disconnected' && status !== 'dead')
  const ready = $derived(active?.kind === 'colab' && (status === 'idle' || status === 'busy'))
  const hostOf = (c: KernelChoice | null) => (c?.kind === 'remote' ? hosts.find((h) => h.label === c.host) : undefined)
  const kernelsOf = (c: KernelChoice) => (c.kind === 'local' ? machine?.kernels : hostOf(c)?.kernels) ?? []
  const describe = (c: KernelChoice) => c.kind === 'colab' ? `Colab ${c.accelerator ?? (c.variant === 'DEFAULT' ? 'CPU' : c.variant)}` : [c.kind === 'local' ? machine?.name : c.host, ...kernelsOf(c).map((k) => k.id)].join(' ')
  const key = (c: KernelChoice | null) => JSON.stringify(c && (c.kind === 'colab' ? [c.kind, c.variant, c.accelerator, c.high_ram] : c.kind === 'remote' ? [c.kind, c.host] : [c.kind]))
  const typeName = (k: KernelType) => (k.label ? `${k.id} · ${k.label}` : k.id)

  const selected = $derived.by((): KernelChoice => {
    if (place === 'local') return { kind: 'local' }
    if (place !== 'colab') return { kind: 'remote', host: place.slice(7) }
    const [variant, accelerator] = runtime.split(':') as [Variant, string?]
    return { kind: 'colab', variant, accelerator: accelerator ?? null, high_ram: highRam }
  })
  const unavailable = $derived(hostOf(selected)?.available === false)
  const current = $derived(key(selected) === key(active) && (live || (active?.kind === 'local' && status === 'disconnected')))
  const name = $derived(active ? describe(active) : 'Not connected')
  const phase = $derived(!active ? '' : hostOf(active)?.available === false ? 'unavailable' : active.kind === 'local' && status === 'disconnected' ? '' : status)
  $effect(() => void (label = name))
  $effect(() => void (detail = active && active.kind !== 'colab' ? kernelsOf(active).map(typeName).join(', ') : active?.high_ram ? 'High RAM' : ''))
  $effect(() => void (kind = active?.kind ?? 'local'))

  let probing: Promise<unknown> | null = null
  const refreshHosts = () => (probing ??= api.hostStatus().then((s) => (hosts = s), () => {}).finally(() => (probing = null)))
  $effect(() => {
    const every = picking ? 3000 : active?.kind === 'remote' ? 10000 : 0
    if (!every) return
    let timer: ReturnType<typeof setTimeout> | undefined
    let stop = false
    const loop = () => refreshHosts().then(() => void (stop || (timer = setTimeout(loop, every))))
    if (picking) loop()
    else timer = setTimeout(loop, every)
    return () => ((stop = true), clearTimeout(timer))
  })

  let now = $state(Date.now())
  $effect(() => {
    started = null
    if (!active || (status !== 'idle' && status !== 'busy')) return
    api.runtimeStarted(path).then((t) => (started = t ? Date.parse(t) : null), () => {})
    const tick = setInterval(() => (now = Date.now()), 1000)
    return () => clearInterval(tick)
  })

  const attempt = (f: () => Promise<unknown>) => f().then(() => (error = ''), (e) => (error = String(e)))
  $effect(() => void (place === 'colab' && !info && attempt(async () => (info = await api.colabInfo()))))

  async function connect() {
    busy = true
    await attempt(async () => {
      if (selected.kind === 'colab' && account.auth.state !== 'signed_in') account.auth = await api.signIn(settings.clientId.trim() || null, settings.clientSecret.trim() || null)
      if (selected.kind === 'colab' && account.auth.state !== 'signed_in') throw new Error('Sign in to Colab first (Settings)')
      await api.selectKernel(path, selected)
    })
    busy = false
  }

  function show(c: KernelChoice) {
    place = c.kind === 'remote' ? `remote:${c.host}` : c.kind
    if (c.kind !== 'colab') return
    runtime = c.accelerator ? `${c.variant}:${c.accelerator}` : c.variant
    highRam = c.high_ram
  }

  onMount(() => {
    const apply = (e: KernelStatusEvent) => {
      if (e.path !== path) return
      if (e.choice && key(e.choice) !== key(active)) show(e.choice)
      status = e.status
      active = e.choice
    }
    const sub = api.onKernelStatus(apply)
    refreshHosts()
    api.machine().then((m) => (machine = m), (e) => (error = String(e)))
    sub.then(() => api.kernelState(path)).then(apply, () => {})
    return () => void sub.then((u) => u())
  })
</script>

<div class="bar">
  {#if error}<span class="error" title={error}>{error}</span>{/if}
  {#if wide && (phase || !active)}<span class="status" class:dead={phase === 'dead'}>{phase || 'Not connected'}</span>{/if}
  {#if wide && ready && started !== null}<span class="status" title="Time since the Colab runtime started">{duration(now - started)}</span>{/if}
  <select bind:value={place} onfocus={() => (picking = true)} onblur={() => (picking = false)} onpointerdown={refreshHosts} aria-label="Kernel location" title="Kernel location">
    <option value="local">{machine?.name ?? ''}</option>
    {#each hosts as h (h.label)}<option value="remote:{h.label}" disabled={!h.available}>{h.label}{h.available ? '' : ' (unavailable)'}</option>{/each}
    <option value="colab">Colab</option>
  </select>
  {#if place === 'colab'}
    <select bind:value={runtime} aria-label="Kernel type" title="Kernel type">
      {#each runtimes as [value, name] (value)}<option {value}>{name}</option>{/each}
    </select>
    <label class="check" title="High RAM"><input type="checkbox" bind:checked={highRam} />{wide ? 'High RAM' : 'RAM+'}</label>
  {:else}
    {@const types = kernelsOf(selected)}
    <select disabled={unavailable} aria-label="Kernel type" title={types.map(typeName).join(', ')}>
      {#each types as k (k.id)}<option>{typeName(k)}</option>{:else}<option>Unknown</option>{/each}
    </select>
  {/if}
  {#if current && live && active?.kind === 'local'}
    <button class="outline" onclick={() => attempt(() => api.detach(path))} title="Shut down the kernel. Variables are lost">Disconnect</button>
  {:else if current && active && active.kind !== 'local'}
    <button class="outline" onclick={() => attempt(() => api.detach(path))} title={active.kind === 'colab' ? 'Disconnect but keep the runtime' : 'Disconnect but keep the kernel running on the host'}>Detach</button>
    <button class="outline" onclick={() => attempt(() => api.release(path))} title={active.kind === 'colab' ? 'Release the runtime' : 'Shut down the kernel on the host. Variables are lost'}>Release</button>
  {:else if !current}
    <button class="primary" onclick={connect} disabled={busy || unavailable || status === 'connecting'}>{busy ? 'Connecting...' : 'Connect'}</button>
  {/if}
</div>

<style>
  .bar { display: flex; align-items: center; gap: 6px; min-width: 0; }
  .bar select, .bar button { height: 32px; }
  .bar select { max-width: 200px; overflow: hidden; text-overflow: ellipsis; padding-left: 13px; font-weight: 500; }
  .bar button { padding: 0 16px; }
  .bar select:disabled { color: var(--faint); cursor: default; }
  .status { color: color-mix(in srgb, var(--fg) 80%, transparent); white-space: nowrap; font-size: 0.923rem; margin: 0 4px; }
  .status.dead { color: var(--error); }
  .check { display: inline-flex; align-items: center; gap: 4px; color: var(--muted); font-size: 0.923rem; white-space: nowrap; }
  .check input { accent-color: var(--accent); }
  .error { color: var(--error); max-width: 220px; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 0.923rem; }
</style>

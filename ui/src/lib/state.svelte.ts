import type { AuthState } from '../bindings/AuthState'
import { inTauri } from './ipc'
import { activeTheme, cssVars, parseSettings } from './settings'

export const settings = $state(parseSettings(localStorage.getItem('settings')))
export const alerts = $state({ error: '' })
export const remote = $state<{ host: string | null }>({ host: null })
export const account = $state<{ auth: AuthState }>({ auth: { state: 'signed_out' } })
export const platform = { mac: /Mac/.test(navigator.userAgent) }

const media = (query: string, set: (on: boolean) => void) => {
  const m = matchMedia(query)
  m.addEventListener('change', (e) => set(e.matches))
  return m.matches
}
export const system = $state({ dark: media('(prefers-color-scheme: dark)', (on) => (system.dark = on)) })
const reduced = $state({ transparency: media('(prefers-reduced-transparency: reduce)', (on) => (reduced.transparency = on)) })

let saved = localStorage.getItem('settings')
window.addEventListener('storage', (e) => {
  if (e.key === 'settings' && e.newValue !== null) Object.assign(settings, parseSettings((saved = e.newValue)))
})

const win = inTauri && platform.mac ? import('@tauri-apps/api/window') : null
const native = { scheme: '', glass: null as boolean | null }
function sync(scheme: 'light' | 'dark' | null, glass: boolean) {
  if (!win) return
  const warn = (e: unknown) => console.warn('window appearance', e)
  if (String(scheme) !== native.scheme) {
    native.scheme = String(scheme)
    win.then((w) => w.getCurrentWindow().setTheme(scheme)).catch(warn)
  }
  if (glass === native.glass) return
  native.glass = glass
  win.then(({ getCurrentWindow, Effect, EffectState }) => glass ? getCurrentWindow().setEffects({ effects: [Effect.LiquidGlassRegular, Effect.Sidebar], state: EffectState.FollowsWindowActiveState }) : getCurrentWindow().clearEffects()).catch(warn)
}

$effect.root(() => {
  $effect(() => {
    const json = JSON.stringify(settings)
    if (json !== saved) localStorage.setItem('settings', (saved = json))
  })
  $effect(() => {
    const root = document.documentElement
    for (const [k, v] of Object.entries(cssVars(settings, system.dark))) root.style.setProperty(k, v)
    const scheme = activeTheme(settings, system.dark).light ? 'light' : 'dark'
    const glass = inTauri && platform.mac && settings.glass && !reduced.transparency
    root.style.colorScheme = scheme
    root.dataset.cursor = settings.cursorStyle
    root.classList.toggle('glass', glass)
    sync(settings.appearance === 'auto' ? null : scheme, glass)
  })
})

import type { AuthState } from '../bindings/AuthState'
import { cssVars, parseSettings } from './settings'

export const settings = $state(parseSettings(localStorage.getItem('settings')))
export const alerts = $state({ error: '' })
export const account = $state<{ auth: AuthState }>({ auth: { state: 'signed_out' } })

$effect.root(() => {
  $effect(() => {
    localStorage.setItem('settings', JSON.stringify(settings))
    for (const [k, v] of Object.entries(cssVars(settings))) document.documentElement.style.setProperty(k, v)
  })
})

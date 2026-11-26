import type { Theme } from '.'
import { fromPalette } from '../palette'

export default {
  name: 'Midnight',
  vars: fromPalette({
    bg: '#060607', panel: '#0c0c0e', editor: '#0e0e11', popover: '#161619', border: '#1c1c21', borderStrong: '#2a2a31',
    fg: '#ececf0', heading: '#ffffff', muted: '#8e8e96', faint: '#5c5c64',
    accent: '#0a84ff', button: '#0071e3', buttonFg: '#ffffff', ok: '#30d158', warn: '#ffd60a', error: '#ff453a',
    kw: '#ff7ab2', str: '#ff8a75', esc: '#ffa14f', num: '#d9c97c', com: '#7f8c98', fn: '#6bdfff', cls: '#dabaff', meta: '#ffa14f', self: '#ff7ab2', builtin: '#acf2e4', op: '#a7a7b0',
    ansi: [
      '#3a3a3f', '#ff453a', '#30d158', '#ffd60a', '#0a84ff', '#bf5af2', '#64d2ff', '#d1d1d6',
      '#6c6c72', '#ff6961', '#4ae17a', '#ffe14d', '#409cff', '#da8fff', '#8ae0ff', '#ffffff',
    ],
  }),
} satisfies Theme

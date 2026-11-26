import type { Theme } from '.'
import { fromPalette } from '../palette'

export default {
  name: 'Rosé Pine',
  vars: fromPalette({
    bg: '#191724', panel: '#1f1d2e', editor: '#191724', popover: '#26233a', border: '#21202e', borderStrong: '#403d52',
    fg: '#e0def4', heading: '#e0def4', muted: '#908caa', faint: '#6e6a86',
    accent: '#c4a7e7', buttonFg: '#191724', ok: '#9ccfd8', warn: '#f6c177', error: '#eb6f92',
    kw: '#31748f', str: '#f6c177', esc: '#31748f', num: '#f6c177', com: '#6e6a86', fn: '#ebbcba', cls: '#9ccfd8', meta: '#c4a7e7', self: '#eb6f92', builtin: '#eb6f92', op: '#908caa',
    ansi: [
      '#26233a', '#eb6f92', '#31748f', '#f6c177', '#9ccfd8', '#c4a7e7', '#ebbcba', '#e0def4',
      '#6e6a86', '#eb6f92', '#31748f', '#f6c177', '#9ccfd8', '#c4a7e7', '#ebbcba', '#e0def4',
    ],
  }),
} satisfies Theme

import type { Theme } from '.'
import { fromPalette } from '../palette'

export default {
  name: 'Solarized Dark',
  vars: fromPalette({
    bg: '#00212b', panel: '#00212b', editor: '#002b36', popover: '#073642', border: '#073642', borderStrong: '#586e75',
    fg: '#839496', heading: '#93a1a1', muted: '#657b83', faint: '#586e75',
    accent: '#268bd2', buttonFg: '#00212b', ok: '#859900', warn: '#b58900', error: '#dc322f',
    kw: '#859900', str: '#2aa198', esc: '#cb4b16', num: '#d33682', com: '#586e75', fn: '#268bd2', cls: '#cb4b16', meta: '#268bd2', self: '#b58900', builtin: '#268bd2', op: '#859900',
    ansi: [
      '#073642', '#dc322f', '#859900', '#b58900', '#268bd2', '#d33682', '#2aa198', '#eee8d5',
      '#002b36', '#cb4b16', '#586e75', '#657b83', '#839496', '#6c71c4', '#93a1a1', '#fdf6e3',
    ],
  }),
} satisfies Theme

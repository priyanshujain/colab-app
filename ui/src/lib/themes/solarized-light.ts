import type { Theme } from '.'
import { fromPalette } from '../palette'

export default {
  name: 'Solarized Light',
  light: true,
  vars: fromPalette({
    light: true,
    bg: '#fdf6e3', panel: '#eee8d5', editor: '#fdf6e3', popover: '#fdf6e3', border: '#ddd6c1', borderStrong: '#93a1a1',
    fg: '#5f747b', heading: '#586e75', muted: '#657b83', faint: '#93a1a1',
    accent: '#268bd2', buttonFg: '#00212b', ok: '#859900', warn: '#b58900', error: '#dc322f',
    kw: '#849700', str: '#299e95', esc: '#cb4b16', num: '#d33682', com: '#93a1a1', fn: '#268bd2', cls: '#cb4b16', meta: '#268bd2', self: '#b38800', builtin: '#268bd2', op: '#849700',
    ansi: [
      '#073642', '#dc322f', '#859900', '#b58900', '#268bd2', '#d33682', '#2aa198', '#eee8d5',
      '#002b36', '#cb4b16', '#586e75', '#657b83', '#839496', '#6c71c4', '#93a1a1', '#fdf6e3',
    ],
  }),
} satisfies Theme

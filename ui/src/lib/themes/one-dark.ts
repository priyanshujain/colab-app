import type { Theme } from '.'
import { fromPalette } from '../palette'

export default {
  name: 'One Dark',
  vars: fromPalette({
    bg: '#21252b', panel: '#21252b', editor: '#282c34', popover: '#21252b', border: '#181a1f', borderStrong: '#3e4452',
    fg: '#abb2bf', heading: '#d7dae0', muted: '#7f848e', faint: '#5c6370',
    accent: '#61afef', buttonFg: '#282c34', ok: '#98c379', warn: '#e5c07b', error: '#e06c75',
    kw: '#c678dd', str: '#98c379', esc: '#56b6c2', num: '#d19a66', com: '#7f848e', fn: '#61afef', cls: '#e5c07b', meta: '#61afef', self: '#e06c75', builtin: '#56b6c2', op: '#56b6c2',
    ansi: [
      '#3f4451', '#e05561', '#8cc265', '#d18f52', '#4aa5f0', '#c162de', '#42b3c2', '#d7dae0',
      '#4f5666', '#ff616e', '#a5e075', '#f0a45d', '#4dc4ff', '#de73ff', '#4cd1e0', '#e6e6e6',
    ],
  }),
} satisfies Theme

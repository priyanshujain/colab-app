import type { Theme } from '.'
import { fromPalette } from '../palette'

export default {
  name: 'Gruvbox Light',
  light: true,
  vars: fromPalette({
    light: true,
    bg: '#f9f5d7', panel: '#fbf1c7', editor: '#fbf1c7', popover: '#f2e5bc', border: '#ebdbb2', borderStrong: '#d5c4a1',
    fg: '#3c3836', heading: '#282828', muted: '#7c6f64', faint: '#a89984',
    accent: '#076678', buttonFg: '#fbf1c7', ok: '#79740e', warn: '#b57614', error: '#9d0006',
    kw: '#9d0006', str: '#79740e', esc: '#af3a03', num: '#8f3f71', com: '#928374', fn: '#79740e', cls: '#b57614', meta: '#427b58', self: '#076678', builtin: '#af3a03', op: '#af3a03',
    ansi: [
      '#fbf1c7', '#cc241d', '#98971a', '#d79921', '#458588', '#b16286', '#689d6a', '#7c6f64',
      '#928374', '#9d0006', '#79740e', '#b57614', '#076678', '#8f3f71', '#427b58', '#3c3836',
    ],
  }),
} satisfies Theme

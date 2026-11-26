import type { Theme } from '.'
import { fromPalette } from '../palette'

export default {
  name: 'Gruvbox Dark',
  vars: fromPalette({
    bg: '#1d2021', panel: '#282828', editor: '#282828', popover: '#3c3836', border: '#3c3836', borderStrong: '#504945',
    fg: '#ebdbb2', heading: '#fbf1c7', muted: '#a89984', faint: '#7c6f64',
    accent: '#83a598', buttonFg: '#282828', ok: '#b8bb26', warn: '#fabd2f', error: '#fb4934',
    kw: '#fb4934', str: '#b8bb26', esc: '#fe8019', num: '#d3869b', com: '#928374', fn: '#b8bb26', cls: '#fabd2f', meta: '#8ec07c', self: '#83a598', builtin: '#fe8019', op: '#fe8019',
    ansi: [
      '#282828', '#cc241d', '#98971a', '#d79921', '#458588', '#b16286', '#689d6a', '#a89984',
      '#928374', '#fb4934', '#b8bb26', '#fabd2f', '#83a598', '#d3869b', '#8ec07c', '#ebdbb2',
    ],
  }),
} satisfies Theme

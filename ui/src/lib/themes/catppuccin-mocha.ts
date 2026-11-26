import type { Theme } from '.'
import { fromPalette } from '../palette'

export default {
  name: 'Catppuccin Mocha',
  vars: fromPalette({
    bg: '#181825', panel: '#181825', editor: '#1e1e2e', popover: '#313244', border: '#11111b', borderStrong: '#45475a',
    fg: '#cdd6f4', heading: '#cdd6f4', muted: '#a6adc8', faint: '#6c7086',
    accent: '#cba6f7', buttonFg: '#11111b', ok: '#a6e3a1', warn: '#f9e2af', error: '#f38ba8',
    kw: '#cba6f7', str: '#a6e3a1', esc: '#f5c2e7', num: '#fab387', com: '#9399b2', fn: '#89b4fa', cls: '#f9e2af', meta: '#fab387', self: '#f38ba8', builtin: '#f38ba8', op: '#89dceb',
    ansi: [
      '#45475a', '#f38ba8', '#a6e3a1', '#f9e2af', '#89b4fa', '#f5c2e7', '#94e2d5', '#bac2de',
      '#585b70', '#f38ba8', '#a6e3a1', '#f9e2af', '#89b4fa', '#f5c2e7', '#94e2d5', '#a6adc8',
    ],
  }),
} satisfies Theme

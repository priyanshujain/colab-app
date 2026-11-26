import type { Theme } from '.'
import { fromPalette } from '../palette'

export default {
  name: 'Catppuccin Latte',
  light: true,
  vars: fromPalette({
    light: true,
    bg: '#e6e9ef', panel: '#e6e9ef', editor: '#eff1f5', popover: '#eff1f5', border: '#dce0e8', borderStrong: '#bcc0cc',
    fg: '#4c4f69', heading: '#4c4f69', muted: '#6c6f85', faint: '#9ca0b0',
    accent: '#8839ef', buttonFg: '#eff1f5', ok: '#40a02b', warn: '#df8e1d', error: '#d20f39',
    kw: '#8839ef', str: '#3f9e2b', esc: '#cb67b0', num: '#ed5d0a', com: '#7c7f93', fn: '#1e66f5', cls: '#c27b19', meta: '#ed5d0a', self: '#d20f39', builtin: '#d20f39', op: '#0494cd',
    ansi: [
      '#5c5f77', '#d20f39', '#40a02b', '#df8e1d', '#1e66f5', '#ea76cb', '#179299', '#acb0be',
      '#6c6f85', '#d20f39', '#40a02b', '#df8e1d', '#1e66f5', '#ea76cb', '#179299', '#bcc0cc',
    ],
  }),
} satisfies Theme

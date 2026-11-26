import type { Theme } from '.'
import { fromPalette } from '../palette'

export default {
  name: 'GitHub Light',
  light: true,
  vars: fromPalette({
    light: true,
    bg: '#f6f8fa', panel: '#f6f8fa', editor: '#ffffff', popover: '#ffffff', border: '#d0d7de', borderStrong: '#afb8c1',
    fg: '#1f2328', heading: '#1f2328', muted: '#656d76', faint: '#8c959f',
    accent: '#0969da', button: '#1f883d', buttonFg: '#ffffff', ok: '#1a7f37', warn: '#9a6700', error: '#d1242f',
    kw: '#cf222e', str: '#0a3069', esc: '#116329', num: '#0550ae', com: '#6e7781', fn: '#8250df', cls: '#953800', meta: '#8250df', self: '#0550ae', builtin: '#0550ae', op: '#cf222e',
    ansi: [
      '#24292f', '#cf222e', '#116329', '#4d2d00', '#0969da', '#8250df', '#1b7c83', '#6e7781',
      '#57606a', '#a40e26', '#1a7f37', '#633c01', '#218bff', '#a475f9', '#3192aa', '#8c959f',
    ],
  }),
} satisfies Theme

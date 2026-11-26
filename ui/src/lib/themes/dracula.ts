import type { Theme } from '.'
import { fromPalette } from '../palette'

export default {
  name: 'Dracula',
  vars: fromPalette({
    bg: '#21222c', panel: '#21222c', editor: '#282a36', popover: '#343746', border: '#191a21', borderStrong: '#44475a',
    fg: '#f8f8f2', heading: '#ffffff', muted: '#a0a4bf', faint: '#6272a4',
    accent: '#bd93f9', buttonFg: '#282a36', ok: '#50fa7b', warn: '#ffb86c', error: '#ff5555',
    kw: '#ff79c6', str: '#f1fa8c', esc: '#ff79c6', num: '#bd93f9', com: '#6272a4', fn: '#50fa7b', cls: '#8be9fd', meta: '#50fa7b', self: '#bd93f9', builtin: '#8be9fd', op: '#ff79c6',
    ansi: [
      '#21222c', '#ff5555', '#50fa7b', '#f1fa8c', '#bd93f9', '#ff79c6', '#8be9fd', '#f8f8f2',
      '#6272a4', '#ff6e6e', '#69ff94', '#ffffa5', '#d6acff', '#ff92df', '#a4ffff', '#ffffff',
    ],
  }),
} satisfies Theme

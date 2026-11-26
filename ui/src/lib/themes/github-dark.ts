import type { Theme } from '.'
import { fromPalette } from '../palette'

export default {
  name: 'GitHub Dark',
  vars: fromPalette({
    bg: '#010409', panel: '#010409', editor: '#0d1117', popover: '#161b22', border: '#30363d', borderStrong: '#484f58',
    fg: '#e6edf3', heading: '#ffffff', muted: '#7d8590', faint: '#6e7681',
    accent: '#2f81f7', button: '#238636', buttonFg: '#ffffff', ok: '#3fb950', warn: '#d29922', error: '#f85149',
    kw: '#ff7b72', str: '#a5d6ff', esc: '#7ee787', num: '#79c0ff', com: '#8b949e', fn: '#d2a8ff', cls: '#ffa657', meta: '#d2a8ff', self: '#79c0ff', builtin: '#79c0ff', op: '#ff7b72',
    ansi: [
      '#484f58', '#ff7b72', '#3fb950', '#d29922', '#58a6ff', '#bc8cff', '#39c5cf', '#b1bac4',
      '#6e7681', '#ffa198', '#56d364', '#e3b341', '#79c0ff', '#d2a8ff', '#56d4dd', '#ffffff',
    ],
  }),
} satisfies Theme

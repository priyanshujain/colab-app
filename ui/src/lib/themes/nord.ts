import type { Theme } from '.'
import { fromPalette } from '../palette'

export default {
  name: 'Nord',
  vars: fromPalette({
    bg: '#2e3440', panel: '#2e3440', editor: '#2e3440', popover: '#3b4252', border: '#3b4252', borderStrong: '#434c5e',
    fg: '#d8dee9', heading: '#eceff4', muted: '#8d97ab', faint: '#616e88',
    accent: '#88c0d0', buttonFg: '#2e3440', ok: '#a3be8c', warn: '#ebcb8b', error: '#bf616a',
    kw: '#81a1c1', str: '#a3be8c', esc: '#ebcb8b', num: '#b48ead', com: '#616e88', fn: '#88c0d0', cls: '#8fbcbb', meta: '#d08770', self: '#81a1c1', builtin: '#88c0d0', op: '#81a1c1',
    ansi: [
      '#3b4252', '#bf616a', '#a3be8c', '#ebcb8b', '#81a1c1', '#b48ead', '#88c0d0', '#e5e9f0',
      '#4c566a', '#bf616a', '#a3be8c', '#ebcb8b', '#81a1c1', '#b48ead', '#8fbcbb', '#eceff4',
    ],
  }),
} satisfies Theme

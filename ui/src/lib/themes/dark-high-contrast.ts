import type { Theme } from '.'

export default {
  name: 'Dark High Contrast',
  vars: {
    '--bg': '#000000', '--panel': '#000000', '--editor-bg': '#000000', '--popover': '#0c141f',
    '--border': '#6fc3df', '--border-strong': '#6fc3df',
    '--fg': '#ffffff', '--code-fg': '#ffffff', '--muted': '#ffffffb3', '--faint': '#a5a5a5', '--heading': '#ffffff',
    '--accent': '#f38518', '--accent-soft': '#f3851833', '--accent-line': '#f38518', '--link': '#21a6ff',
    '--button-bg': '#000000', '--button-fg': '#ffffff', '--button-hover': '#000000', '--button-border': '#6fc3df',
    '--selection': '#008000', '--active-line': 'transparent', '--match': '#ffffff33', '--hover': '#ffffff1a', '--hover-strong': '#ffffff26',
    '--ok': '#89d185', '--warn': '#ffd370', '--error': '#f48771',
    '--inline-code': '#ffffff1a', '--scrollbar': '#6fc3df99', '--scrollbar-hover': '#6fc3dfcc',
    '--tk-kw': '#c586c0', '--tk-import': '#c586c0', '--tk-storage': '#569cd6', '--tk-opkw': '#569cd6', '--tk-str': '#ce9178', '--tk-esc': '#569cd6',
    '--tk-num': '#b5cea8', '--tk-const': '#569cd6', '--tk-com': '#7ca668', '--tk-com-style': 'normal', '--tk-fn': '#dcdcaa', '--tk-def': '#dcdcaa', '--tk-def-weight': 'normal',
    '--tk-cls': '#4ec9b0', '--tk-var': '#9cdcfe', '--tk-prop': '#9cdcfe', '--tk-op': '#d4d4d4', '--tk-punc': '#ffffff', '--tk-meta': '#dcdcaa',
    '--tk-self': '#9cdcfe', '--tk-self-style': 'normal', '--tk-builtin': '#dcdcaa', '--tk-fmt': '#569cd6',
    '--out-stderr': '#f4877120', '--out-json': '#9cdcfe', '--out-row-alt': 'transparent', '--out-row-hover': '#ffffff1a',
    '--out-input': '#000000', '--out-input-border': '#6fc3df', '--out-layout': '#0c141f', '--out-strong': '#6fc3df', '--out-handle': '#ffffff',
    '--ansi-black': '#000000', '--ansi-red': '#cd0000', '--ansi-green': '#00cd00', '--ansi-yellow': '#cdcd00',
    '--ansi-blue': '#0000ee', '--ansi-magenta': '#cd00cd', '--ansi-cyan': '#00cdcd', '--ansi-white': '#e5e5e5',
    '--ansi-bright-black': '#7f7f7f', '--ansi-bright-red': '#ff0000', '--ansi-bright-green': '#00ff00', '--ansi-bright-yellow': '#ffff00',
    '--ansi-bright-blue': '#5c5cff', '--ansi-bright-magenta': '#ff00ff', '--ansi-bright-cyan': '#00ffff', '--ansi-bright-white': '#ffffff',
    '--ansi-red-bg': '#cd000060', '--ansi-green-bg': '#00cd0060', '--ansi-yellow-bg': '#cdcd0060', '--ansi-blue-bg': '#0000ee80',
  },
} satisfies Theme

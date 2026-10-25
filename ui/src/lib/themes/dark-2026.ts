import type { Theme } from '.'

export default {
  name: 'Dark 2026',
  vars: {
    '--bg': '#121314', '--panel': '#191a1b', '--editor-bg': '#191a1b', '--popover': '#202122',
    '--border': '#2a2b2c', '--border-strong': '#333536',
    '--fg': '#bfbfbf', '--code-fg': '#bbbebf', '--muted': '#8c8c8c', '--faint': '#555555', '--heading': '#ededed',
    '--accent': '#3994bc', '--accent-soft': '#3994bc33', '--accent-line': '#3994bcb3', '--link': '#48a0c7',
    '--button-bg': '#297aa0', '--button-fg': '#ffffff', '--button-hover': '#2b7da3', '--button-border': '#297aa0',
    '--selection': '#276782dd', '--active-line': '#ffffff0b', '--match': '#27678260', '--hover': '#ffffff14', '--hover-strong': '#ffffff22',
    '--ok': '#73c991', '--warn': '#cca700', '--error': '#f48771',
    '--inline-code': '#262626', '--scrollbar': '#a8a9aa85', '--scrollbar-hover': '#a8a9aa90',
    '--tk-kw': '#c586c0', '--tk-import': '#c586c0', '--tk-storage': '#ff7b72', '--tk-opkw': '#569cd6', '--tk-str': '#a5d6ff', '--tk-esc': '#d7ba7d',
    '--tk-num': '#b5cea8', '--tk-const': '#569cd6', '--tk-com': '#8b949e', '--tk-com-style': 'normal', '--tk-fn': '#d2a8ff', '--tk-def': '#d2a8ff', '--tk-def-weight': 'normal',
    '--tk-cls': '#4ec9b0', '--tk-var': '#c9d1d9', '--tk-prop': '#c9d1d9', '--tk-op': '#d4d4d4', '--tk-punc': '#bbbebf', '--tk-meta': '#d2a8ff',
    '--tk-self': '#79c0ff', '--tk-self-style': 'normal', '--tk-builtin': '#dcdcaa', '--tk-fmt': '#ff7b72',
    '--out-stderr': '#f4877114', '--out-json': '#9cdcfe', '--out-row-alt': '#ffffff08', '--out-row-hover': '#ffffff14',
    '--out-input': '#191a1b', '--out-input-border': '#333536', '--out-layout': '#202122', '--out-strong': '#333536', '--out-handle': '#878889',
    '--ansi-black': '#000000', '--ansi-red': '#cd3131', '--ansi-green': '#0dbc79', '--ansi-yellow': '#e5e510',
    '--ansi-blue': '#2472c8', '--ansi-magenta': '#bc3fbc', '--ansi-cyan': '#11a8cd', '--ansi-white': '#e5e5e5',
    '--ansi-bright-black': '#666666', '--ansi-bright-red': '#f14c4c', '--ansi-bright-green': '#23d18b', '--ansi-bright-yellow': '#f5f543',
    '--ansi-bright-blue': '#3b8eea', '--ansi-bright-magenta': '#d670d6', '--ansi-bright-cyan': '#29b8db', '--ansi-bright-white': '#e5e5e5',
    '--ansi-red-bg': '#cd313140', '--ansi-green-bg': '#0dbc7940', '--ansi-yellow-bg': '#e5e51040', '--ansi-blue-bg': '#2472c840',
  },
} satisfies Theme

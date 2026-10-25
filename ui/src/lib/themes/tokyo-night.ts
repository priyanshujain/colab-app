import type { Theme } from '.'

export default {
  name: 'Tokyo Night',
  vars: {
    '--bg': '#1a1b26', '--panel': '#16161e', '--editor-bg': '#16161e', '--popover': '#16161e',
    '--border': '#101014', '--border-strong': '#29355a',
    '--fg': '#a9b1d6', '--code-fg': '#a9b1d6', '--muted': '#787c99', '--faint': '#545c7e', '--heading': '#c0caf5',
    '--accent': '#7aa2f7', '--accent-soft': '#3d59a14d', '--accent-line': '#3d59a1', '--link': '#6183bb',
    '--button-bg': '#3d59a1dd', '--button-fg': '#ffffff', '--button-hover': '#3d59a1aa', '--button-border': 'transparent',
    '--selection': '#515c7e4d', '--active-line': '#8890c00c', '--match': '#515c7e44', '--hover': '#13131a', '--hover-strong': '#202330',
    '--ok': '#9ece6a', '--warn': '#e0af68', '--error': '#db4b4b',
    '--inline-code': '#16161e', '--scrollbar': '#868bc415', '--scrollbar-hover': '#868bc425',
    '--tk-kw': '#bb9af7', '--tk-import': '#7dcfff', '--tk-storage': '#bb9af7', '--tk-opkw': '#bb9af7', '--tk-str': '#9ece6a', '--tk-esc': '#89ddff',
    '--tk-num': '#ff9e64', '--tk-const': '#ff9e64', '--tk-com': '#51597d', '--tk-com-style': 'italic', '--tk-fn': '#7aa2f7', '--tk-def': '#7aa2f7', '--tk-def-weight': 'normal',
    '--tk-cls': '#c0caf5', '--tk-var': '#c0caf5', '--tk-prop': '#73daca', '--tk-op': '#89ddff', '--tk-punc': '#89ddff', '--tk-meta': '#7aa2f7',
    '--tk-self': '#f7768e', '--tk-self-style': 'normal', '--tk-builtin': '#2ac3de', '--tk-fmt': '#bb9af7',
    '--out-stderr': '#85353e33', '--out-json': '#7aa2f7', '--out-row-alt': '#1c1d29', '--out-row-hover': '#202330',
    '--out-input': '#14141b', '--out-input-border': '#0f0f14', '--out-layout': '#16161e', '--out-strong': '#29355a', '--out-handle': '#acb0d0',
    '--ansi-black': '#363b54', '--ansi-red': '#f7768e', '--ansi-green': '#73daca', '--ansi-yellow': '#e0af68',
    '--ansi-blue': '#7aa2f7', '--ansi-magenta': '#bb9af7', '--ansi-cyan': '#7dcfff', '--ansi-white': '#787c99',
    '--ansi-bright-black': '#363b54', '--ansi-bright-red': '#f7768e', '--ansi-bright-green': '#73daca', '--ansi-bright-yellow': '#e0af68',
    '--ansi-bright-blue': '#7aa2f7', '--ansi-bright-magenta': '#bb9af7', '--ansi-bright-cyan': '#7dcfff', '--ansi-bright-white': '#acb0d0',
    '--ansi-red-bg': '#f7768e33', '--ansi-green-bg': '#73daca33', '--ansi-yellow-bg': '#e0af6833', '--ansi-blue-bg': '#7aa2f733',
  },
} satisfies Theme

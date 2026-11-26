import type { Theme } from './themes'

type Base = 'bg' | 'panel' | 'editor' | 'popover' | 'border' | 'borderStrong' | 'fg' | 'heading' | 'muted' | 'faint' | 'accent' | 'buttonFg' | 'ok' | 'warn' | 'error'
type Syntax = 'kw' | 'str' | 'esc' | 'num' | 'com' | 'fn' | 'cls' | 'meta' | 'self' | 'builtin' | 'op'
export type Palette = Record<Base | Syntax, string> & { light?: boolean; button?: string; ansi: string[] }

const ansiNames = ['black', 'red', 'green', 'yellow', 'blue', 'magenta', 'cyan', 'white']

export function fromPalette(p: Palette): Theme['vars'] {
  const o = p.light ? '#000000' : '#ffffff'
  const button = p.button ?? p.accent
  return {
    '--bg': p.bg, '--panel': p.panel, '--editor-bg': p.editor, '--popover': p.popover,
    '--border': p.border, '--border-strong': p.borderStrong,
    '--fg': p.fg, '--code-fg': p.fg, '--muted': p.muted, '--faint': p.faint, '--heading': p.heading,
    '--accent': p.accent, '--accent-soft': p.accent + '26', '--accent-line': p.accent + '99', '--link': p.accent,
    '--button-bg': button, '--button-fg': p.buttonFg, '--button-hover': `color-mix(in srgb, ${button} 85%, ${o})`, '--button-border': 'transparent',
    '--selection': p.accent + (p.light ? '33' : '40'), '--active-line': o + (p.light ? '08' : '06'), '--match': o + '14', '--hover': o + '0a', '--hover-strong': o + '14',
    '--ok': p.ok, '--warn': p.warn, '--error': p.error,
    '--inline-code': o + '0f', '--scrollbar': o + (p.light ? '26' : '1a'), '--scrollbar-hover': o + (p.light ? '40' : '2e'),
    '--tk-kw': p.kw, '--tk-import': p.kw, '--tk-storage': p.kw, '--tk-opkw': p.kw, '--tk-str': p.str, '--tk-esc': p.esc,
    '--tk-num': p.num, '--tk-const': p.num, '--tk-com': p.com, '--tk-com-style': 'italic', '--tk-fn': p.fn, '--tk-def': p.fn, '--tk-def-weight': 'normal',
    '--tk-cls': p.cls, '--tk-var': p.fg, '--tk-prop': p.fg, '--tk-op': p.op, '--tk-punc': p.fg, '--tk-meta': p.meta,
    '--tk-self': p.self, '--tk-self-style': 'italic', '--tk-builtin': p.builtin, '--tk-fmt': p.esc,
    '--out-stderr': p.error + '14', '--out-json': p.accent, '--out-row-alt': o + '08', '--out-row-hover': p.accent + '1f',
    '--out-input': p.editor, '--out-input-border': p.borderStrong, '--out-layout': p.popover, '--out-strong': p.borderStrong, '--out-handle': p.muted,
    ...Object.fromEntries(p.ansi.map((c, i) => [`--ansi-${i < 8 ? '' : 'bright-'}${ansiNames[i % 8]}`, c])),
    '--ansi-red-bg': p.ansi[1] + '40', '--ansi-green-bg': p.ansi[2] + '40', '--ansi-yellow-bg': p.ansi[3] + '40', '--ansi-blue-bg': p.ansi[4] + '40',
  } as Theme['vars']
}

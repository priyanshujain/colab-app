import { themes, type Theme } from './themes'

export const defaults = {
  interfaceFont: 'Inter',
  interfaceSize: 13,
  markdownFont: 'Inter',
  markdownSize: 14,
  codeFont: 'JetBrains Mono',
  codeSize: 13,
  lineHeight: 1.6,
  ligatures: false,
  tabSize: 4,
  lineNumbers: false,
  wordWrap: true,
  theme: 'default',
  lightTheme: 'github-light',
  appearance: 'dark' as 'dark' | 'light' | 'auto',
  accent: '',
  glass: true,
  opacity: 100,
  padding: 12,
  radius: 8,
  cursorStyle: 'bar' as 'bar' | 'block' | 'underline',
  cursorBlink: true,
  dblclickMarkdown: false,
  lintDiagnostics: true,
  typeDiagnostics: true,
  keys: {
    rename: 'Mod+R',
    formatCell: 'Alt+Shift+F',
    formatNotebook: 'Mod+Alt+Shift+F',
    organize: 'Alt+Shift+O',
    quickFix: 'Mod+.',
    find: 'Mod+F',
    definition: 'F12',
    runCell: 'Mod+Enter',
    runAll: 'Mod+Shift+Enter',
  },
  clientId: '',
  clientSecret: '',
}

export type Settings = typeof defaults

const choices = { appearance: ['dark', 'light', 'auto'], cursorStyle: ['bar', 'block', 'underline'] }
const ranges = { opacity: [60, 100], padding: [0, 48], radius: [0, 16] }

export function parseSettings(raw: string | null): Settings {
  let saved: Record<string, unknown> = {}
  try {
    saved = JSON.parse(raw ?? '{}') ?? {}
  } catch {}
  saved = { codeFont: saved.fontFamily, codeSize: saved.fontSize, interfaceSize: saved.uiFontSize, ...saved }
  const s = { ...defaults } as Record<string, unknown>
  for (const k of Object.keys(defaults)) if (typeof saved[k] === typeof s[k] && k !== 'keys') s[k] = saved[k]
  const keys = (saved.keys ?? {}) as Record<string, unknown>
  for (const [k, list] of Object.entries(choices)) if (!list.includes(s[k] as string)) s[k] = defaults[k as keyof typeof choices]
  for (const [k, [lo, hi]] of Object.entries(ranges)) s[k] = Math.min(hi, Math.max(lo, s[k] as number))
  if (!/^#[0-9a-f]{6}$/i.test(s.accent as string)) s.accent = ''
  s.keys = Object.fromEntries(Object.entries(defaults.keys).map(([k, v]) => [k, typeof keys[k] === 'string' ? keys[k] : v]))
  return s as Settings
}

const sans = (font: string) => `${JSON.stringify(font.trim() || 'Inter')}, 'Inter Variable', -apple-system, BlinkMacSystemFont, sans-serif`

export const activeTheme = (s: Settings, systemDark: boolean): Theme =>
  (s.appearance === 'auto' ? systemDark : s.appearance === 'dark') ? (themes[s.theme] ?? themes.default) : (themes[s.lightTheme] ?? themes['github-light'] ?? themes.default)

function accentVars(hex: string) {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4))
  const l = 0.2126 * r + 0.7152 * g + 0.0722 * b
  const bright = (l + 0.05) / 0.05 > 1.05 / (l + 0.05)
  return { '--accent': hex, '--accent-soft': `${hex}26`, '--accent-line': `${hex}99`, '--link': hex, '--button-bg': hex, '--button-hover': `color-mix(in srgb, ${hex} 85%, ${bright ? 'white' : 'black'})`, '--button-fg': bright ? '#000000' : '#ffffff' }
}

export const cssVars = (s: Settings, systemDark = true) => ({
  ...activeTheme(s, systemDark).vars,
  ...(s.accent ? accentVars(s.accent) : {}),
  '--mono': `${JSON.stringify(s.codeFont.trim() || defaults.codeFont)}, 'JetBrains Mono', 'SF Mono', Menlo, monospace`,
  '--code-size': `${s.codeSize}px`,
  '--code-line-height': String(s.lineHeight),
  '--code-ligatures': s.ligatures ? 'normal' : 'none',
  '--sans': sans(s.interfaceFont),
  '--ui-size': `${s.interfaceSize}px`,
  '--md-font': sans(s.markdownFont),
  '--md-size': `${s.markdownSize}px`,
  '--radius': `${s.radius}px`,
  '--pad': `${s.padding}px`,
  '--opacity': String(s.opacity),
})

export type Action = keyof Settings['keys']

export const actions: [Action, string][] = [
  ['rename', 'Rename symbol'],
  ['formatCell', 'Format cell'],
  ['formatNotebook', 'Format notebook'],
  ['organize', 'Organize imports'],
  ['quickFix', 'Quick fix'],
  ['find', 'Find'],
  ['definition', 'Go to definition'],
  ['runCell', 'Run cell'],
  ['runAll', 'Run all'],
]

const names: Record<string, string> = { Period: '.', Comma: ',', Slash: '/', Semicolon: ';', Quote: "'", BracketLeft: '[', BracketRight: ']', Backslash: '\\', Minus: '-', Equal: '=', Backquote: '`', Space: 'Space' }

export function combo(e: KeyboardEvent) {
  if (['Meta', 'Control', 'Alt', 'Shift'].includes(e.key)) return null
  const key = names[e.code] ?? e.code.replace(/^(Key|Digit|Numpad)/, '')
  return [e.metaKey || e.ctrlKey ? 'Mod' : '', e.altKey ? 'Alt' : '', e.shiftKey ? 'Shift' : '', key].filter(Boolean).join('+')
}

export const pressed = (e: KeyboardEvent, keys: Settings['keys'], action: Action) => combo(e) === keys[action]

export const label = (c: string) => c.replace(/Mod\+?/g, '\u2318').replace(/Alt\+?/g, '\u2325').replace(/Shift\+?/g, '\u21e7').replace('Enter', '\u21a9')

import { themes } from './themes'

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
  dblclickMarkdown: false,
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

export function parseSettings(raw: string | null): Settings {
  let saved: Record<string, unknown> = {}
  try {
    saved = JSON.parse(raw ?? '{}') ?? {}
  } catch {}
  saved = { codeFont: saved.fontFamily, codeSize: saved.fontSize, interfaceSize: saved.uiFontSize, ...saved }
  const s = { ...defaults } as Record<string, unknown>
  for (const k of Object.keys(defaults)) if (typeof saved[k] === typeof s[k] && k !== 'keys') s[k] = saved[k]
  const keys = (saved.keys ?? {}) as Record<string, unknown>
  s.keys = Object.fromEntries(Object.entries(defaults.keys).map(([k, v]) => [k, typeof keys[k] === 'string' ? keys[k] : v]))
  return s as Settings
}

const sans = (font: string) => `${JSON.stringify(font.trim() || 'Inter')}, 'Inter Variable', -apple-system, BlinkMacSystemFont, sans-serif`

export const cssVars = (s: Settings) => ({
  ...(themes[s.theme] ?? themes.default).vars,
  '--mono': `${JSON.stringify(s.codeFont.trim() || defaults.codeFont)}, 'JetBrains Mono', 'SF Mono', Menlo, monospace`,
  '--code-size': `${s.codeSize}px`,
  '--code-line-height': String(s.lineHeight),
  '--code-ligatures': s.ligatures ? 'normal' : 'none',
  '--sans': sans(s.interfaceFont),
  '--ui-size': `${s.interfaceSize}px`,
  '--md-font': sans(s.markdownFont),
  '--md-size': `${s.markdownSize}px`,
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

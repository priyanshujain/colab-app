import { expect, test } from 'vitest'
import { activeTheme, combo, cssVars, defaults, label, parseSettings, pressed } from './settings'
import { themes } from './themes'

test('saved settings keep valid values and fall back on corrupt or mistyped ones', () => {
  expect(parseSettings('{"fontSize":')).toEqual(defaults)
  const s = parseSettings(JSON.stringify({ codeSize: 15, tabSize: '2', wordWrap: false, codeFont: 'Fira Code', stale: 1 }))
  expect(s).toEqual({ ...defaults, codeSize: 15, wordWrap: false, codeFont: 'Fira Code' })
  expect(cssVars(s)['--mono']).toMatch(/^"Fira Code", /)
  expect(cssVars({ ...s, codeFont: ' ' })['--mono']).toMatch(/^"JetBrains Mono", /)
})

test('settings saved before the font split keep the chosen code font and sizes', () => {
  const s = parseSettings(JSON.stringify({ fontFamily: 'Monaco', fontSize: 14, uiFontSize: 12, lineHeight: 1.4, theme: 'default' }))
  expect(s).toEqual({ ...defaults, codeFont: 'Monaco', codeSize: 14, interfaceSize: 12, lineHeight: 1.4 })
  expect(parseSettings(JSON.stringify(s))).toEqual(s)
  expect(cssVars(s)).toMatchObject({ '--mono': expect.stringMatching(/^"Monaco", /), '--code-size': '14px', '--ui-size': '12px', '--md-size': '14px', '--sans': expect.stringMatching(/^"Inter", /) })
})

test('shortcuts match by physical key so Option combos work, and saved rebinds survive reload', () => {
  const key = (init: KeyboardEventInit) => ({ code: 'KeyF', key: 'Ï', altKey: false, shiftKey: false, metaKey: false, ctrlKey: false, ...init }) as KeyboardEvent
  expect(combo(key({ altKey: true, shiftKey: true }))).toBe('Alt+Shift+F')
  expect(pressed(key({ altKey: true, shiftKey: true }), defaults.keys, 'formatCell')).toBe(true)
  expect(pressed(key({ altKey: true, shiftKey: true, metaKey: true }), defaults.keys, 'formatCell')).toBe(false)
  expect(pressed(key({ altKey: true, shiftKey: true, metaKey: true }), defaults.keys, 'formatNotebook')).toBe(true)
  expect(combo(key({ code: 'Period', key: '.', metaKey: true }))).toBe('Mod+.')
  expect(combo(key({ code: 'MetaLeft', key: 'Meta', metaKey: true }))).toBeNull()
  const s = parseSettings(JSON.stringify({ keys: { rename: 'F2', find: 7, bogus: 'X' } }))
  expect(s.keys).toEqual({ ...defaults.keys, rename: 'F2' })
  expect(label(s.keys.runAll)).toBe('⌘⇧↩')
})

test('appearance picks the dark or light theme, auto follows the system, and removed theme ids fall back', () => {
  const s = parseSettings(JSON.stringify({ theme: 'tokyo-night', lightTheme: 'solarized-light', appearance: 'auto' }))
  expect(activeTheme(s, true)).toBe(themes['tokyo-night'])
  expect(activeTheme(s, false)).toBe(themes['solarized-light'])
  expect(cssVars(s, false)['--bg']).toBe(themes['solarized-light'].vars['--bg'])
  expect(cssVars(s, true)['--bg']).toBe(themes['tokyo-night'].vars['--bg'])
  expect(activeTheme({ ...s, appearance: 'dark' }, false)).toBe(themes['tokyo-night'])
  expect(activeTheme({ ...s, appearance: 'light' }, true)).toBe(themes['solarized-light'])
  const removed = { ...s, theme: 'monokai-2019', lightTheme: 'paper' }
  expect(activeTheme({ ...removed, appearance: 'dark' }, false)).toBe(themes.default)
  expect(activeTheme({ ...removed, appearance: 'light' }, true)).toBe(themes['github-light'])
  expect(activeTheme({ ...removed, appearance: 'light' }, true).light).toBe(true)
  expect(activeTheme({ ...removed, appearance: 'dark' }, true).light).toBeFalsy()
})

test('settings saved before appearance options existed keep their dark theme even when the system is light', () => {
  const old = { interfaceFont: 'Inter', interfaceSize: 13, markdownFont: 'Georgia', markdownSize: 15, codeFont: 'Fira Code', codeSize: 14, lineHeight: 1.5, ligatures: true, tabSize: 2, lineNumbers: true, wordWrap: false, theme: 'tokyo-night', dblclickMarkdown: true, lintDiagnostics: true, typeDiagnostics: false, keys: { ...defaults.keys, rename: 'F2' }, clientId: '', clientSecret: '' }
  const s = parseSettings(JSON.stringify(old))
  expect(s).toEqual({ ...defaults, ...old })
  expect(cssVars(s, false)).toMatchObject({ '--bg': themes['tokyo-night'].vars['--bg'], '--accent': themes['tokyo-night'].vars['--accent'], '--mono': expect.stringMatching(/^"Fira Code", /), '--pad': '12px', '--radius': '8px', '--opacity': '100' })
})

const luminance = (hex: string) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4)).reduce((sum, c, i) => sum + c * [0.2126, 0.7152, 0.0722][i], 0)
const contrast = (a: string, b: string) => { const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x); return (hi + 0.05) / (lo + 0.05) }

test('a custom accent recolors every accent variable and keeps button text readable on any accent', () => {
  const s = parseSettings(JSON.stringify({ theme: 'default', accent: '#FFC600' }))
  expect(cssVars(s)).toMatchObject({ '--accent': '#FFC600', '--accent-soft': '#FFC60026', '--accent-line': '#FFC60099', '--link': '#FFC600', '--button-bg': '#FFC600', '--button-fg': '#000000', '--button-hover': expect.stringContaining('#FFC600') })
  expect(cssVars({ ...s, accent: '#5b2a86' })['--button-fg']).toBe('#ffffff')
  expect(cssVars({ ...s, accent: '' })['--accent']).toBe(themes.default.vars['--accent'])
  expect(cssVars({ ...s, accent: '', appearance: 'auto' }, false)['--accent']).toBe(themes['github-light'].vars['--accent'])
  for (const accent of ['#007aff', '#a550a7', '#f74f9e', '#ff5257', '#f7821b', '#ffc600', '#62ba46', '#8c8c8c', '#767676', '#777777', '#1e1e1e', '#ffffff']) {
    const v = cssVars({ ...s, accent })
    expect(contrast(v['--button-fg'], accent), accent).toBeGreaterThanOrEqual(4.5)
  }
})

test('invalid enum values, malformed accents and out of range sizes from a hand edited file are corrected', () => {
  const s = parseSettings(JSON.stringify({ appearance: 'sepia', cursorStyle: 'beam', accent: 'red', glass: 'yes', opacity: 20, padding: 120, radius: -4, cursorBlink: false, lightTheme: 'gruvbox-light' }))
  expect(s).toMatchObject({ appearance: 'dark', cursorStyle: 'bar', accent: '', glass: true, opacity: 60, padding: 48, radius: 0, cursorBlink: false, lightTheme: 'gruvbox-light' })
  expect(parseSettings(JSON.stringify({ accent: '#abc', opacity: '80', appearance: 'light', cursorStyle: 'underline' }))).toMatchObject({ accent: '', opacity: 100, appearance: 'light', cursorStyle: 'underline' })
  expect(cssVars(s)).toMatchObject({ '--opacity': '60', '--pad': '48px', '--radius': '0px' })
})

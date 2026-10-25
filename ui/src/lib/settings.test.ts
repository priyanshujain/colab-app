import { expect, test } from 'vitest'
import { combo, cssVars, defaults, label, parseSettings, pressed } from './settings'

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

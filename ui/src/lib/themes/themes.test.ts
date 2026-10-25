import { expect, test } from 'vitest'
import { themes } from '.'
import { cssVars, parseSettings } from '../settings'

test('every theme defines exactly the default variables and settings apply the chosen one', () => {
  expect(Object.keys(themes)).toEqual(['default', 'dark-2026', 'dark-high-contrast', 'tokyo-night'])
  const keys = Object.keys(themes.default.vars).sort()
  for (const t of Object.values(themes)) expect(Object.keys(t.vars).sort()).toEqual(keys)
  expect(cssVars(parseSettings('{"theme":"tokyo-night"}'))['--bg']).toBe('#1a1b26')
  expect(cssVars(parseSettings('{"theme":"gone"}'))['--bg']).toBe(themes.default.vars['--bg'])
})

test('active line is translucent in every theme so the selection layer underneath stays visible', () => {
  for (const t of Object.values(themes)) expect(t.vars['--active-line'], t.name).toMatch(/^(transparent|#[0-9a-f]{6}[0-9a-f]{2})$/)
})

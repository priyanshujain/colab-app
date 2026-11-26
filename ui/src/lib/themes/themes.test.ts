import { expect, test } from 'vitest'
import { themes } from '.'
import { cssVars, parseSettings } from '../settings'

test('every theme defines exactly the default variables, themes are ordered default, dark, light, and settings apply the chosen one', () => {
  expect(Object.keys(themes)).toEqual([
    'default', 'catppuccin-mocha', 'dark-2026', 'dark-high-contrast', 'dracula', 'github-dark', 'gruvbox-dark', 'midnight', 'nord', 'one-dark', 'rose-pine', 'solarized-dark', 'tokyo-night',
    'catppuccin-latte', 'github-light', 'gruvbox-light', 'solarized-light',
  ])
  const keys = Object.keys(themes.default.vars).sort()
  for (const t of Object.values(themes)) expect(Object.keys(t.vars).sort(), t.name).toEqual(keys)
  expect(cssVars(parseSettings('{"theme":"tokyo-night"}'))['--bg']).toBe('#1a1b26')
  expect(cssVars(parseSettings('{"theme":"gone"}'))['--bg']).toBe(themes.default.vars['--bg'])
})

test('active line is translucent in every theme so the selection layer underneath stays visible', () => {
  for (const t of Object.values(themes)) expect(t.vars['--active-line'], t.name).toMatch(/^(transparent|#[0-9a-f]{6}[0-9a-f]{2})$/)
})

const rgba = (hex: string) => hex.slice(1).match(/../g)!.map((h) => parseInt(h, 16) / 255)
const over = (fg: string, bg: number[]) => { const [r, g, b, a = 1] = rgba(fg); return [r, g, b].map((c, i) => c * a + bg[i] * (1 - a)) }
const lum = (c: number[]) => c.map((v) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4)).reduce((s, v, i) => s + v * [0.2126, 0.7152, 0.0722][i], 0)
const ratio = (fg: string, bg: string, base: string) => {
  const back = over(bg, rgba(base))
  const [hi, lo] = [lum(over(fg, back)), lum(back)].sort((x, y) => y - x)
  return (hi + 0.05) / (lo + 0.05)
}

test('text, code tokens and buttons meet WCAG contrast in every theme', () => {
  const failures: string[] = []
  for (const [id, t] of Object.entries(themes)) {
    const v = t.vars
    const tokens = Object.keys(v).filter((k) => k.startsWith('--tk-') && v[k as keyof typeof v].startsWith('#'))
    const checks: [string, string, number][] = [
      ['--fg', '--bg', 4.5], ['--fg', '--editor-bg', 4.5], ['--code-fg', '--editor-bg', 4.5], ['--muted', '--bg', 3], ['--muted', '--editor-bg', 3], ['--button-fg', '--button-bg', 4.5],
      ...tokens.map((k): [string, string, number] => [k, '--editor-bg', ['--tk-com', '--tk-punc', '--tk-fmt'].includes(k) ? 2 : 3]),
    ]
    for (const [fg, bg, min] of checks) {
      const r = ratio(v[fg as keyof typeof v], v[bg as keyof typeof v], v['--bg'])
      if (r < min) failures.push(`${id} ${fg} on ${bg}: ${r.toFixed(2)} < ${min}`)
    }
  }
  expect(failures).toEqual([])
})

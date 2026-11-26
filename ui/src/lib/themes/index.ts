import base from './default'

export type Theme = { name: string; light?: boolean; vars: Record<keyof typeof base.vars, string> }

const files = import.meta.glob<{ default: Theme }>(['./*.ts', '!./index.ts', '!./*.test.ts'], { eager: true })

export const themes: Record<string, Theme> = Object.fromEntries(
  Object.entries(files).map(([path, m]) => [path.slice(2, -3), m.default] as const).sort(([a, x], [b, y]) => Number(b === 'default') - Number(a === 'default') || Number(!!x.light) - Number(!!y.light) || x.name.localeCompare(y.name)),
)

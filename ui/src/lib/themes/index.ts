import base from './default'

export type Theme = { name: string; vars: Record<keyof typeof base.vars, string> }

const files = import.meta.glob<{ default: Theme }>(['./*.ts', '!./index.ts', '!./*.test.ts'], { eager: true })

export const themes: Record<string, Theme> = Object.fromEntries(
  Object.entries(files).map(([path, m]) => [path.slice(2, -3), m.default]).sort(([a], [b]) => Number(b === 'default') - Number(a === 'default')),
)

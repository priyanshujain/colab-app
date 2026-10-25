import { expect, test } from 'vitest'
import { findAll, pattern, replaceAll, replaceAt } from './find'

const cells = [
  'import torch\nimport torch.nn as nn\n\ntorch.manual_seed(0)',
  '## Policy network\n\nA tiny `nn.Module` that maps the state to action logits.',
  'class Policy(nn.Module):\n    def forward(self, obs: torch.Tensor):\n        return self.net(obs)',
]
const opts = { caseSensitive: false, wholeWord: false, regex: false }
const at = (re: RegExp | null) => findAll(cells, re).map((m) => [m.cell, cells[m.cell].slice(m.from, m.to)])

test('find walks every cell in order and honours case, whole word and regex toggles', () => {
  expect(at(pattern('torch', opts))).toEqual([[0, 'torch'], [0, 'torch'], [0, 'torch'], [2, 'torch']])
  expect(at(pattern('nn.', opts))).toEqual([[1, 'nn.'], [2, 'nn.']])
  expect(at(pattern('policy', opts))).toEqual([[1, 'Policy'], [2, 'Policy']])
  expect(at(pattern('policy', { ...opts, caseSensitive: true }))).toEqual([])
  expect(at(pattern('nn', { ...opts, wholeWord: true }))).toEqual([[0, 'nn'], [0, 'nn'], [1, 'nn'], [2, 'nn']])
  expect(at(pattern('obs\\b', { ...opts, regex: true }))).toEqual([[2, 'obs'], [2, 'obs']])
  expect(at(pattern('^\\s*', { ...opts, regex: true })).every(([, t]) => t)).toBe(true)
  expect(pattern('(', { ...opts, regex: true })).toBeNull()
  expect(pattern('', opts)).toBeNull()
})

test('replace edits one match or every match, with groups only in regex mode', () => {
  const re = pattern('torch', opts)!
  const [, second] = findAll(cells, re)
  expect(replaceAt(cells[0], second, re, 'jax', false)).toBe('import torch\nimport jax.nn as nn\n\ntorch.manual_seed(0)')
  expect(replaceAll(cells[0], re, '$&2', false)).toBe('import $&2\nimport $&2.nn as nn\n\n$&2.manual_seed(0)')
  const obs = pattern('(\\w+): torch\\.Tensor', { ...opts, regex: true })!
  expect(replaceAll(cells[2], obs, '$1', true)).toContain('def forward(self, obs):')
  expect(replaceAt(cells[2], findAll(cells, obs)[0], obs, '$1: "Tensor"', true)).toContain('obs: "Tensor"')
})

import { Text } from '@codemirror/state'
import { expect, test } from 'vitest'
import { highlightPython } from './highlight'
import { completions, diagnostics, hoverText, renameRange, shown, target } from './lsp'
import { renderMarkdown } from './markdown'

test('definition targets map ty locations to a cell of this notebook or a library file', () => {
  const nb = '/Users/me/My Project/train.ipynb'
  const cell = [{ targetUri: 'vscode-notebook-cell:///Users/me/My%20Project/train.ipynb#a1b2', targetRange: {}, targetSelectionRange: { start: { line: 2, character: 4 }, end: { line: 2, character: 9 } } }]
  expect(target(cell, nb)).toEqual({ cell: 'a1b2', line: 2, character: 4 })
  expect(target(cell, '/Users/me/other.ipynb')).toBeNull()
  const lib = { uri: 'file:///Users/me/My%20Project/.venv/lib/python3.12/site-packages/numpy/__init__.py', range: { start: { line: 40, character: 0 }, end: { line: 40, character: 6 } } }
  expect(target(lib, nb)).toEqual({ file: '/Users/me/My Project/.venv/lib/python3.12/site-packages/numpy/__init__.py', line: 40, character: 0 })
  expect(target(null, nb)).toBeNull()
  expect(target([], nb)).toBeNull()
})

test('completion list keeps server order and uses insert text', () => {
  const out = completions({ isIncomplete: false, items: [
    { label: 'mean', kind: 2, detail: '(axis=None) -> float' },
    { label: 'max(...)', kind: 3, insertText: 'max', documentation: { kind: 'markdown', value: 'Largest item' } },
  ] })
  expect(out.map((c) => [c.label, c.apply, c.type, c.detail])).toEqual([['mean', 'mean', 'method', '(axis=None) -> float'], ['max(...)', 'max', 'function', undefined]])
  expect(out[0].boost! > out[1].boost!).toBe(true)
})

test('hover contents in every LSP shape become markdown', () => {
  expect(hoverText({ contents: { kind: 'markdown', value: '```python\nx: int\n```' } })).toBe('```python\nx: int\n```')
  expect(hoverText({ contents: [{ language: 'python', value: 'def f() -> None' }, 'Docs'] })).toBe('```python\ndef f() -> None\n```\n\nDocs')
  expect(hoverText(null)).toBe('')
})

test('diagnostics are placed by cell local line and character and clamped to the doc', () => {
  const doc = Text.of(['import os', 'x = undefined_name'])
  const out = diagnostics([
    { range: { start: { line: 0, character: 7 }, end: { line: 0, character: 9 } }, severity: 2, source: 'Ruff', code: 'F401', message: '`os` imported but unused' },
    { range: { start: { line: 1, character: 4 }, end: { line: 5, character: 0 } }, severity: 1, source: 'ty', message: 'Name `undefined_name` used when not defined' },
  ], doc)
  expect(out.map((d) => [doc.sliceString(d.from, d.to), d.severity, d.source])).toEqual([['os', 'warning', 'Ruff F401'], ['undefined_name', 'error', 'ty']])
  expect(out.map((d) => d.actions?.map((a) => a.name))).toEqual([['Quick Fix...'], ['Quick Fix...']])
})

test('python highlighting marks self, builtins, decorators and definitions', () => {
  const html = highlightPython('@dataclass\nclass A:\n    def run(self, n: int) -> list:\n        return [len(str(i)) for i in range(n)]  # ok')
  for (const span of ['tk-meta">@', 'tk-meta">dataclass', 'tk-cls">A', 'tk-def">run', 'tk-self">self', 'tk-builtin">int', 'tk-builtin">len', 'tk-kw">return', 'tk-com"># ok'])
    expect(html).toContain(span)
  expect(highlightPython('x = "<b>"')).toContain('&lt;b&gt;')
})

test('markdown renders inline and display math, tables and highlighted code', () => {
  const html = renderMarkdown('Rate $\\eta = 10^{-3}$ costs $5.\n\n$$\\sum_i x_i$$\n\n| a |\n| - |\n| 1 |\n\n```python\nprint(1)\n```')
  expect(html.match(/class="katex"/g)).toHaveLength(2)
  expect(html).toContain('katex-display')
  expect(html).toContain('<table>')
  expect(html).toContain('<span class="tk-builtin">print</span>')
})

test('YouTube embeds and bare YouTube links render as cards that open the watch page', () => {
  const html = renderMarkdown('<iframe width="640" height="360" src="https://www.youtube-nocookie.com/embed/kCc8FmEb1nY?start=90&amp;rel=0" title="Let\'s build GPT" allowfullscreen></iframe>\n\nhttps://youtu.be/dQw4w9WgXcQ?t=1m5s\n\n[docs](https://docs.python.org/3/)')
  expect(html).not.toContain('<iframe')
  expect(html).toContain('<a class="yt-card" href="https://www.youtube.com/watch?v=kCc8FmEb1nY&t=90s" title="Watch on YouTube" style="width: min(640px, 100%)"><img src="https://i.ytimg.com/vi/kCc8FmEb1nY/hqdefault.jpg"')
  expect(html).toContain('<span class="yt-title">Let\'s build GPT</span>')
  expect(html).toContain('href="https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=65s"')
  expect(html).toContain('<a href="https://docs.python.org/3/">docs</a>')
  expect(renderMarkdown('<iframe src="https://example.com/embed/abcdefgh"></iframe>')).toContain('<iframe')
})

test('rename range comes from prepareRename in each LSP shape', () => {
  const doc = Text.of(['for step in range(steps):', '    print(step * lr)'])
  const pos = doc.line(2).from + 18
  expect(renameRange({ start: { line: 1, character: 17 }, end: { line: 1, character: 19 } }, doc, pos)).toEqual({ from: pos - 1, to: pos + 1, name: 'lr' })
  expect(renameRange({ range: { start: { line: 1, character: 17 }, end: { line: 1, character: 19 } }, placeholder: 'lr' }, doc, pos)?.name).toBe('lr')
  expect(renameRange({ defaultBehavior: true }, doc, pos)).toEqual({ from: pos - 1, to: pos + 1, name: 'lr' })
  expect(renameRange(null, doc, pos)).toBeNull()
})

test('diagnostics follow the lint and type settings and drop missing imports on colab kernels', () => {
  const range = { start: { line: 0, character: 0 }, end: { line: 0, character: 9 } }
  const list = [
    { range, severity: 2, source: 'Ruff', code: 'I001', message: 'Import block is un-sorted or un-formatted' },
    { range, severity: 1, source: 'ty', code: 'unresolved-import', message: 'Cannot resolve imported module `google.colab`' },
    { range, severity: 1, source: 'ty', code: 'unresolved-reference', message: 'Name `missing` used when not defined' },
  ]
  const codes = (lint: boolean, types: boolean, colab: boolean) => shown(list, { lintDiagnostics: lint, typeDiagnostics: types }, colab).map((d) => (d as { code: string }).code)
  expect(codes(true, true, false)).toEqual(['I001', 'unresolved-import', 'unresolved-reference'])
  expect(codes(true, true, true)).toEqual(['I001', 'unresolved-reference'])
  expect(codes(false, true, false)).toEqual(['unresolved-import', 'unresolved-reference'])
  expect(codes(true, false, false)).toEqual(['I001'])
})

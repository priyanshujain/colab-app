import { expect, test } from 'vitest'
import type { Cell } from '../bindings/Cell'
import { applyEdit } from './edit'
import { offsets, visibleRange } from './virtual'

const cells: Cell[] = [
  { id: 'a', cell_type: 'markdown', source: '# Title', metadata: {} },
  { id: 'b', cell_type: 'code', source: 'x = 1', metadata: { tags: ['setup'] }, execution_count: 3, outputs: [{ output_type: 'stream', name: 'stdout', text: '1\n' }] },
  { id: 'c', cell_type: 'code', source: 'print(x)', metadata: {}, execution_count: null, outputs: [] },
]

test('move, type change and delete keep ids, source and metadata', () => {
  const moved = applyEdit(cells, { kind: 'move', cell_id: 'c', index: 0 })
  expect(moved.map((c) => c.id)).toEqual(['c', 'a', 'b'])
  const md = applyEdit(moved, { kind: 'change_type', cell_id: 'b', cell_type: 'markdown' })
  expect(md[2]).toEqual({ id: 'b', cell_type: 'markdown', source: 'x = 1', metadata: { tags: ['setup'] } })
  const back = applyEdit(md, { kind: 'change_type', cell_id: 'b', cell_type: 'code' })
  expect(back[2]).toMatchObject({ cell_type: 'code', source: 'x = 1', outputs: [], execution_count: null })
  expect(applyEdit(back, { kind: 'delete', cell_id: 'a' }).map((c) => c.id)).toEqual(['c', 'b'])
})

test('visible range of a 300 cell notebook only covers the viewport plus overscan', () => {
  const offs = offsets(Array.from({ length: 300 }, (_, i) => (i % 5 === 0 ? 80 : 60)))
  expect(visibleRange(offs, 0, 600, 2)).toEqual([0, 12])
  const [from, to] = visibleRange(offs, offs[150] + 10, 600, 2)
  expect([from, to]).toEqual([148, 162])
  expect(visibleRange(offs, offs[300] - 600, 600, 2)[1]).toBe(300)
})

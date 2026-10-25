import { expect, test } from 'vitest'
import type { Cell } from '../bindings/Cell'
import { duration, lastRun } from './timing'

const cell = (execution: Record<string, string>, outputs: Cell['outputs'] = []): Cell => ({ id: 'a', cell_type: 'code', source: 'x', metadata: { execution }, execution_count: 1, outputs })
const saved = { 'iopub.status.busy': '2026-10-16T09:00:00.100000Z', 'iopub.execute_input': '2026-10-16T09:00:00.102000Z', 'shell.execute_reply': '2026-10-16T09:01:03.600000Z', 'iopub.status.idle': '2026-10-16T09:01:03.598000Z' }

test('durations read like the VS Code gutter', () => {
  expect([0, 420, 9960, 12400, 63000, 3725000].map(duration)).toEqual(['0.0s', '0.4s', '9.9s', '12s', '1m 3s', '1h 2m'])
})

test('saved JupyterLab timing gives duration and status, errors and interrupted runs fail', () => {
  expect(lastRun(cell(saved))).toEqual({ start: Date.parse(saved['iopub.status.busy']), ms: 63498, ok: true })
  expect(lastRun(cell(saved, [{ output_type: 'error', ename: 'KeyboardInterrupt', evalue: '', traceback: [] }]))?.ok).toBe(false)
  const { 'shell.execute_reply': _, ...cancelled } = saved
  expect(lastRun(cell(cancelled))?.ok).toBe(false)
  expect(lastRun(cell({ 'iopub.status.busy': saved['iopub.status.busy'] }))?.ms).toBeNull()
  expect(lastRun({ ...cell({}), metadata: {} })).toBeNull()
})

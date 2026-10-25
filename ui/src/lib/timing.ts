import type { Cell } from '../bindings/Cell'

export const duration = (ms: number) => {
  const s = Math.max(0, ms) / 1000
  if (s < 10) return `${(Math.floor(s * 10) / 10).toFixed(1)}s`
  if (s < 60) return `${Math.floor(s)}s`
  const m = Math.floor(s / 60)
  return m < 60 ? `${m}m ${Math.floor(s % 60)}s` : `${Math.floor(m / 60)}h ${m % 60}m`
}

export const lastRun = (cell: Cell) => {
  const t = cell.metadata.execution as Record<string, string> | undefined
  const start = Date.parse(t?.['iopub.status.busy'] ?? t?.['iopub.execute_input'] ?? '')
  const end = Date.parse(t?.['iopub.status.idle'] ?? t?.['shell.execute_reply'] ?? '')
  if (isNaN(start)) return null
  const ok = !!t?.['shell.execute_reply'] && !cell.outputs?.some((o) => o.output_type === 'error')
  return { start, ms: isNaN(end) ? null : end - start, ok }
}

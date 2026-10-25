import type { Cell } from '../bindings/Cell'
import type { CellEdit } from '../bindings/CellEdit'

export const newCell = (cell_type: Cell['cell_type'], id = crypto.randomUUID().slice(0, 8)): Cell =>
  cell_type === 'code'
    ? { id, cell_type, source: '', metadata: {}, execution_count: null, outputs: [] }
    : { id, cell_type, source: '', metadata: {} }

export function applyEdit(cells: Cell[], edit: CellEdit): Cell[] {
  const at = (id: string) => cells.findIndex((c) => c.id === id)
  const next = [...cells]
  switch (edit.kind) {
    case 'source':
      return cells.map((c) => (c.id === edit.cell_id ? { ...c, source: edit.source } : c))
    case 'insert':
      next.splice(edit.index, 0, newCell(edit.cell_type, edit.cell_id))
      return next
    case 'delete':
      return cells.filter((c) => c.id !== edit.cell_id)
    case 'move': {
      const i = at(edit.cell_id)
      if (i < 0) return cells
      const [c] = next.splice(i, 1)
      next.splice(Math.max(0, Math.min(edit.index, next.length)), 0, c)
      return next
    }
    case 'change_type':
      return cells.map((c) =>
        c.id !== edit.cell_id || c.cell_type === edit.cell_type ? c : { ...newCell(edit.cell_type, c.id), source: c.source, metadata: c.metadata },
      )
    case 'focus':
      return cells
  }
}

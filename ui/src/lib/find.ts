import { StateEffect, StateField } from '@codemirror/state'
import { Decoration, EditorView } from '@codemirror/view'

export type FindOptions = { caseSensitive: boolean; wholeWord: boolean; regex: boolean }
export type Match = { cell: number; from: number; to: number }

export function pattern(query: string, o: FindOptions): RegExp | null {
  if (!query) return null
  const body = o.regex ? query : query.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')
  try {
    return new RegExp(o.wholeWord ? `\\b(?:${body})\\b` : body, o.caseSensitive ? 'gm' : 'gim')
  } catch {
    return null
  }
}

export const findAll = (sources: string[], re: RegExp | null): Match[] =>
  re ? sources.flatMap((s, cell) => [...s.matchAll(re)].filter((m) => m[0]).map((m) => ({ cell, from: m.index, to: m.index + m[0].length }))) : []

export const replaceAll = (source: string, re: RegExp, with_: string, regex: boolean) =>
  regex ? source.replace(re, with_) : source.replace(re, () => with_)

export const replaceAt = (source: string, m: Match, re: RegExp, with_: string, regex: boolean) =>
  source.slice(0, m.from) + replaceAll(source.slice(m.from, m.to), new RegExp(re.source, re.flags.replace('g', '')), with_, regex) + source.slice(m.to)

const setMarks = StateEffect.define<{ from: number; to: number }[]>()
const mark = Decoration.mark({ class: 'cm-searchMatch' })
export const searchField = StateField.define({
  create: () => Decoration.none,
  update: (deco, tr) => {
    for (const e of tr.effects) if (e.is(setMarks)) return Decoration.set(e.value.map((m) => mark.range(m.from, m.to)))
    return deco.map(tr.changes)
  },
  provide: (f) => EditorView.decorations.from(f),
})

export const searchMarks = (view: EditorView, marks: Match[]) =>
  setMarks.of(marks.filter((m) => m.to <= view.state.doc.length))

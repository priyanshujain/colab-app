import { autocompletion, type Completion, type CompletionContext } from '@codemirror/autocomplete'
import { linter, lintGutter, type Diagnostic } from '@codemirror/lint'
import { Facet, StateEffect, StateField, type Text } from '@codemirror/state'
import { Decoration, EditorView, hoverTooltip, keymap, showTooltip, ViewPlugin, type Tooltip, type ViewUpdate } from '@codemirror/view'
import type { CodeActionItem } from '../bindings/CodeActionItem'
import { renderMarkdown } from './markdown'
import { pressed, type Settings } from './settings'

export interface Lsp {
  completion(line: number, character: number): Promise<unknown>
  hover(line: number, character: number): Promise<unknown>
  definition(line: number, character: number): void
  format(): void
  organize(): void
  codeActions(line: number, character: number): Promise<CodeActionItem[]>
  applyAction(id: number): void
  prepareRename(line: number, character: number): Promise<unknown>
  rename(line: number, character: number, name: string): void
}

type Range = { start: { line: number; character: number }; end: { line: number; character: number } }
type Any = Record<string, any>

const kinds: Record<number, string> = { 2: 'method', 3: 'function', 4: 'function', 5: 'property', 6: 'variable', 7: 'class', 9: 'namespace', 10: 'property', 14: 'keyword', 21: 'constant', 22: 'class' }

const text = (v: unknown): string =>
  typeof v === 'string' ? v
  : Array.isArray(v) ? v.map(text).filter(Boolean).join('\n\n')
  : v && typeof v === 'object' ? ('language' in v ? '```python\n' + (v as Any).value + '\n```' : String((v as Any).value ?? ''))
  : ''

export function completions(result: unknown): Completion[] {
  const items: Any[] = Array.isArray(result) ? result : ((result as Any)?.items ?? [])
  return items.map((i, n) => ({
    label: i.label,
    apply: i.textEdit?.newText ?? i.insertText ?? i.label,
    type: kinds[i.kind],
    detail: i.detail ?? i.labelDetails?.description,
    info: i.documentation ? () => html(text(i.documentation)) : undefined,
    boost: 99 - Math.min(n, 198),
  }))
}

export const hoverText = (result: unknown) => text((result as Any)?.contents).trim()

export type Target = { cell: string; line: number; character: number } | { file: string; line: number; character: number }

export function target(result: unknown, notebook: string): Target | null {
  const loc: Any | undefined = Array.isArray(result) ? result[0] : (result as Any) ?? undefined
  if (!loc) return null
  const uri: string = loc.targetUri ?? loc.uri
  const { line, character } = (loc.targetSelectionRange ?? loc.range).start
  const [file, cell] = decodeURIComponent(uri.replace(/^(file|vscode-notebook-cell):(\/\/)?/, '')).split('#')
  if (uri.startsWith('vscode-notebook-cell:')) return file === notebook ? { cell, line, character } : null
  return { file, line, character }
}

const offset = (doc: Text, p: Range['start']) => {
  if (p.line >= doc.lines) return doc.length
  const line = doc.line(p.line + 1)
  return Math.min(line.from + p.character, line.to)
}

export function diagnostics(list: unknown[], doc: Text): Diagnostic[] {
  return (list as Any[]).map((d) => {
    const from = offset(doc, d.range.start)
    return {
      from,
      to: Math.max(from, offset(doc, d.range.end)),
      severity: d.severity === 1 ? 'error' : d.severity === 2 ? 'warning' : d.severity === 4 ? 'hint' : 'info',
      message: d.message,
      source: [d.source, d.code].filter(Boolean).join(' '),
      actions: [{ name: 'Quick Fix...', apply: (view: EditorView) => void quickFix(view, from) }],
    }
  })
}

function html(markdown: string) {
  const dom = document.createElement('div')
  dom.className = 'lsp-doc markdown'
  dom.innerHTML = renderMarkdown(markdown)
  return dom
}

const at = (doc: Text, pos: number) => {
  const line = doc.lineAt(pos)
  return [line.number - 1, pos - line.from] as const
}

export function lspExtensions(lsp: Lsp, keys: () => Settings['keys']) {
  const complete = async (ctx: CompletionContext) => {
    const word = ctx.matchBefore(/\w*/)
    const after = ctx.state.sliceDoc(ctx.pos - 1, ctx.pos)
    if (!ctx.explicit && !word?.text && after !== '.') return null
    const result = await lsp.completion(...at(ctx.state.doc, ctx.pos))
    return { from: word?.from ?? ctx.pos, options: completions(result), validFor: /^\w*$/ }
  }
  const go = (view: EditorView, pos: number) => (lsp.definition(...at(view.state.doc, pos)), true)
  return [
    autocompletion({ override: [complete], icons: false }),
    hoverTooltip(async (view, pos) => {
      const t = hoverText(await lsp.hover(...at(view.state.doc, pos)).catch(() => null))
      return t ? { pos, above: true, create: () => ({ dom: html(t) }) } : null
    }, { hideOnChange: true }),
    lspFacet.of(lsp),
    fixMenu,
    renameBox,
    linkHover,
    EditorView.domEventHandlers({
      keydown: (e, view) => {
        const head = view.state.selection.main.head
        const run = { formatCell: () => lsp.format(), organize: () => lsp.organize(), definition: () => go(view, head), quickFix: () => quickFix(view, head), rename: () => startRename(view, head) }
        const hit = (Object.keys(run) as (keyof typeof run)[]).find((a) => pressed(e, keys(), a))
        return !!hit && (e.preventDefault(), run[hit](), true)
      },
      mousedown: (e, view) => {
        if (!(e.metaKey || e.ctrlKey)) return false
        const pos = view.posAtCoords(e)
        return pos != null && (e.preventDefault(), go(view, pos))
      },
    }),
    linter(null),
    lintGutter(),
  ]
}

const lspFacet = Facet.define<Lsp, Lsp | undefined>({ combine: (v) => v[0] })
const showFix = StateEffect.define<{ pos: number; items: CodeActionItem[] } | null>()
const fixMenu = StateField.define<Tooltip | null>({
  create: () => null,
  update: (tip, tr) => {
    for (const e of tr.effects) if (e.is(showFix)) tip = e.value && { pos: e.value.pos, above: false, create: (view) => fixDom(view, e.value!.items) }
    return tr.docChanged || (tr.selection && !tr.effects.some((e) => e.is(showFix))) ? null : tip
  },
  provide: (f) => showTooltip.from(f),
})

export async function quickFix(view: EditorView, pos: number) {
  const lsp = view.state.facet(lspFacet)
  if (!lsp) return
  const items = await lsp.codeActions(...at(view.state.doc, pos)).catch(() => [])
  view.dispatch({ effects: showFix.of({ pos, items }) })
}

function fixDom(view: EditorView, items: CodeActionItem[]) {
  const dom = document.createElement('div')
  dom.className = 'cm-quickfix'
  const close = () => (view.dispatch({ effects: showFix.of(null) }), view.contentDOM.focus({ preventScroll: true }))
  if (!items.length) dom.textContent = 'No quick fixes available'
  for (const item of items) {
    const b = dom.appendChild(document.createElement('button'))
    b.textContent = item.title
    b.onclick = () => (close(), view.state.facet(lspFacet)?.applyAction(item.id))
  }
  dom.onkeydown = (e) => {
    const buttons = [...dom.querySelectorAll('button')]
    const i = buttons.indexOf(document.activeElement as HTMLButtonElement)
    if (e.key === 'Escape') close()
    else if (e.key === 'ArrowDown' || e.key === 'ArrowUp') buttons[(i + (e.key === 'ArrowDown' ? 1 : buttons.length - 1)) % buttons.length]?.focus()
    else return
    e.preventDefault()
    e.stopPropagation()
  }
  return { dom, mount: () => dom.querySelector('button')?.focus({ preventScroll: true }) }
}

const showRename = StateEffect.define<{ from: number; to: number; name: string } | null>()
const renameBox = StateField.define<Tooltip | null>({
  create: () => null,
  update: (tip, tr) => {
    for (const e of tr.effects) if (e.is(showRename)) tip = e.value && { pos: e.value.from, above: true, create: (view) => renameDom(view, e.value!) }
    return tr.docChanged || (tr.selection && !tr.effects.some((e) => e.is(showRename))) ? null : tip
  },
  provide: (f) => showTooltip.from(f),
})

export function renameRange(result: unknown, doc: Text, pos: number) {
  const r = result as Any | null
  const range: Range | undefined = r?.range ?? (r?.start ? r : undefined)
  const word = range ? { from: offset(doc, range.start), to: offset(doc, range.end) } : r?.defaultBehavior ? wordAround(doc, pos) : null
  return word ? { ...word, name: r?.placeholder ?? doc.sliceString(word.from, word.to) } : null
}

const wordAround = (doc: Text, pos: number) => {
  const line = doc.lineAt(pos)
  const [a, b] = [line.text.slice(0, pos - line.from).match(/\w*$/)![0], line.text.slice(pos - line.from).match(/^\w*/)![0]]
  return a + b ? { from: pos - a.length, to: pos + b.length } : null
}

async function startRename(view: EditorView, pos: number) {
  const lsp = view.state.facet(lspFacet)
  const range = lsp && renameRange(await lsp.prepareRename(...at(view.state.doc, pos)).catch(() => null), view.state.doc, pos)
  if (range) view.dispatch({ effects: showRename.of(range) })
}

function renameDom(view: EditorView, range: { from: number; name: string }) {
  const dom = document.createElement('div')
  dom.className = 'cm-rename'
  const input = dom.appendChild(document.createElement('input'))
  input.value = range.name
  input.spellcheck = false
  input.setAttribute('aria-label', 'New name')
  const close = () => (view.dispatch({ effects: showRename.of(null) }), view.focus())
  input.onkeydown = (e) => {
    e.stopPropagation()
    if (e.key === 'Escape') close()
    if (e.key !== 'Enter') return
    e.preventDefault()
    const name = input.value.trim()
    const where = at(view.state.doc, range.from)
    close()
    if (name && name !== range.name) view.state.facet(lspFacet)?.rename(...where, name)
  }
  input.onblur = () => dom.isConnected && view.dispatch({ effects: showRename.of(null) })
  return { dom, mount: () => input.select() }
}

const setLink = StateEffect.define<{ from: number; to: number } | null>()
const link = StateField.define({
  create: () => Decoration.none,
  update: (deco, tr) => {
    for (const e of tr.effects) if (e.is(setLink)) deco = e.value ? Decoration.set(Decoration.mark({ class: 'cm-def-link' }).range(e.value.from, e.value.to)) : Decoration.none
    return tr.docChanged ? Decoration.none : deco
  },
  provide: (f) => EditorView.decorations.from(f),
})

const linkHover = [
  link,
  ViewPlugin.fromClass(
    class {
      mouse: MouseEvent | null = null
      range = ''
      constructor(readonly view: EditorView) {
        window.addEventListener('keydown', this.key)
        window.addEventListener('keyup', this.key)
        window.addEventListener('blur', this.clear)
      }
      key = (e: KeyboardEvent) => this.mouse && this.show(this.mouse, e.metaKey || e.ctrlKey)
      clear = () => this.show(null, false)
      show(e: MouseEvent | null, mod: boolean) {
        this.mouse = e
        const pos = e && mod ? this.view.posAtCoords(e) : null
        const word = pos == null ? null : this.view.state.wordAt(pos)
        const range = word ? `${word.from}:${word.to}` : ''
        if (range === this.range) return
        this.range = range
        this.view.dispatch({ effects: setLink.of(word && { from: word.from, to: word.to }) })
      }
      update(u: ViewUpdate) {
        if (u.docChanged) this.range = ''
      }
      destroy() {
        window.removeEventListener('keydown', this.key)
        window.removeEventListener('keyup', this.key)
        window.removeEventListener('blur', this.clear)
      }
    },
    {
      eventHandlers: {
        mousemove(e) { this.show(e, e.metaKey || e.ctrlKey) },
        mouseleave() { this.clear() },
      },
    },
  ),
]

const flash = StateEffect.define<number | null>()
export const flashField = StateField.define({
  create: () => Decoration.none,
  update: (deco, tr) => {
    for (const e of tr.effects) if (e.is(flash)) deco = e.value == null ? Decoration.none : Decoration.set(Decoration.line({ class: 'cm-flash' }).range(e.value))
    return deco.map(tr.changes)
  },
  provide: (f) => EditorView.decorations.from(f),
})

export function flashLine(view: EditorView, pos: number) {
  const from = view.state.doc.lineAt(pos).from
  view.dispatch({ effects: flash.of(from) })
  setTimeout(() => view.dom.isConnected && view.dispatch({ effects: flash.of(null) }), 1200)
}

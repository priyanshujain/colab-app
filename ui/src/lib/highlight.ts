import { pythonLanguage } from '@codemirror/lang-python'
import { syntaxTree } from '@codemirror/language'
import type { EditorState } from '@codemirror/state'
import { Decoration, ViewPlugin, type DecorationSet, type ViewUpdate } from '@codemirror/view'
import { highlightTree, tagHighlighter, tags as t } from '@lezer/highlight'
import type { SyntaxNode, Tree } from '@lezer/common'

const highlighter = tagHighlighter([
  { tag: [t.keyword, t.controlKeyword], class: 'tk-kw' },
  { tag: t.moduleKeyword, class: 'tk-import' },
  { tag: [t.definitionKeyword, t.modifier], class: 'tk-storage' },
  { tag: t.operatorKeyword, class: 'tk-opkw' },
  { tag: [t.string, t.special(t.string)], class: 'tk-str' },
  { tag: t.escape, class: 'tk-esc' },
  { tag: t.number, class: 'tk-num' },
  { tag: [t.bool, t.null], class: 'tk-const' },
  { tag: t.variableName, class: 'tk-var' },
  { tag: t.lineComment, class: 'tk-com' },
  { tag: [t.function(t.variableName), t.function(t.propertyName)], class: 'tk-fn' },
  { tag: t.function(t.definition(t.variableName)), class: 'tk-def' },
  { tag: [t.definition(t.className), t.className], class: 'tk-cls' },
  { tag: t.propertyName, class: 'tk-prop' },
  { tag: [t.arithmeticOperator, t.bitwiseOperator, t.compareOperator, t.updateOperator, t.definitionOperator], class: 'tk-op' },
  { tag: t.meta, class: 'tk-meta' },
  { tag: [t.paren, t.squareBracket, t.brace, t.separator, t.derefOperator, t.punctuation], class: 'tk-punc' },
])

const builtins = new Set(
  ('abs all any ascii bin bool breakpoint bytearray bytes callable chr classmethod compile complex delattr dict dir divmod enumerate eval exec filter float format frozenset getattr globals hasattr hash help hex id input int isinstance issubclass iter len list locals map max memoryview min next object oct open ord pow print property range repr reversed round set setattr slice sorted staticmethod str sum super tuple type vars zip __import__ __name__ __file__ ' +
    'Exception BaseException ValueError TypeError KeyError IndexError AttributeError RuntimeError StopIteration NotImplementedError ImportError OSError FileNotFoundError ZeroDivisionError AssertionError KeyboardInterrupt NotImplemented Ellipsis').split(' '),
)

const inDecorator = (n: SyntaxNode | null): boolean => !!n && n.name !== 'ArgList' && (n.name === 'Decorator' || inDecorator(n.parent))

function extra(n: SyntaxNode, word: string): string | null {
  if (n.name === 'from') return 'tk-import'
  if ((n.name === '{' || n.name === '}') && n.parent?.name === 'FormatReplacement') return 'tk-fmt'
  if (n.name !== 'VariableName' && n.name !== 'PropertyName') return null
  if (inDecorator(n.parent)) return 'tk-meta'
  if (n.name !== 'VariableName' || n.parent?.name === 'FunctionDefinition' || n.parent?.name === 'ClassDefinition') return null
  if (word === 'self' || word === 'cls') return 'tk-self'
  return builtins.has(word) ? 'tk-builtin' : null
}

export function tokens(tree: Tree, text: string): [number, number, string][] {
  const out = new Map<number, [number, number, string]>()
  highlightTree(tree, highlighter, (from, to, cls) => out.set(from, [from, to, cls]))
  tree.iterate({ enter: (n) => {
    const cls = extra(n.node, text.slice(n.from, n.to))
    if (cls) out.set(n.from, [n.from, n.to, cls])
  } })
  return [...out.values()].sort((a, b) => a[0] - b[0])
}

export const escape = (s: string) => s.replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]!)

export function highlightPython(code: string): string {
  let html = '', at = 0
  for (const [from, to, cls] of tokens(pythonLanguage.parser.parse(code), code)) {
    if (from < at) continue
    html += escape(code.slice(at, from)) + `<span class="${cls}">${escape(code.slice(from, to))}</span>`
    at = to
  }
  return html + escape(code.slice(at))
}

const build = (state: EditorState): DecorationSet =>
  Decoration.set(tokens(syntaxTree(state), state.doc.toString()).map(([f, to, c]) => Decoration.mark({ class: c }).range(f, to)), true)

export const pythonHighlight = ViewPlugin.fromClass(
  class {
    decorations: DecorationSet
    constructor(view: { state: EditorState }) { this.decorations = build(view.state) }
    update(u: ViewUpdate) {
      if (u.docChanged || syntaxTree(u.state) !== syntaxTree(u.startState)) this.decorations = build(u.state)
    }
  },
  { decorations: (v) => v.decorations },
)

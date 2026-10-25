<script lang="ts">
  import { closeBrackets, closeBracketsKeymap, completionKeymap } from '@codemirror/autocomplete'
  import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
  import { python } from '@codemirror/lang-python'
  import { bracketMatching, indentOnInput, indentUnit } from '@codemirror/language'
  import { setDiagnostics } from '@codemirror/lint'
  import { highlightSelectionMatches, searchKeymap } from '@codemirror/search'
  import { searchField, searchMarks, type Match } from './find'
  import { Compartment, EditorState, Prec } from '@codemirror/state'
  import { drawSelection, EditorView, highlightActiveLine, highlightActiveLineGutter, keymap, lineNumbers } from '@codemirror/view'
  import { onMount } from 'svelte'
  import { pythonHighlight } from './highlight'
  import { editorTheme } from './theme'
  import { diagnostics as toDiagnostics, flashField, flashLine, lspExtensions, type Lsp } from './lsp'
  import { settings } from './state.svelte'
  import { pressed } from './settings'

  let { source, language, focused = false, readonly = false, lsp, diagnostics = [], marks = [], match = null, reveal = null, onchange, onfocus, onkey }: {
    source: string
    language: 'python' | 'markdown'
    focused?: boolean
    readonly?: boolean
    lsp?: Lsp
    diagnostics?: unknown[]
    marks?: Match[]
    match?: Match | null
    reveal?: { line: number; character: number } | null
    onchange?: (s: string) => void
    onfocus?: () => void
    onkey?: (key: 'Shift-Enter' | 'Mod-Enter' | 'Escape') => void
  } = $props()

  let host: HTMLDivElement
  let view = $state.raw<EditorView>()
  let external = false
  const options = new Compartment()
  const configure = () => [
    indentUnit.of(' '.repeat(settings.tabSize)),
    EditorState.tabSize.of(settings.tabSize),
    readonly || settings.lineNumbers ? [lineNumbers(), highlightActiveLineGutter()] : [],
    settings.wordWrap ? EditorView.lineWrapping : [],
  ]

  onMount(() => {
    const keys = (['Shift-Enter', 'Escape'] as const).map((key) => ({ key, run: () => (onkey?.(key), !!onkey) }))
    const run = EditorView.domEventHandlers({ keydown: (e) => !!onkey && pressed(e, settings.keys, 'runCell') && (e.preventDefault(), onkey('Mod-Enter'), true) })
    const v = new EditorView({
      parent: host,
      doc: source,
      extensions: [
        Prec.highest([keymap.of(keys), run]),
        readonly ? EditorState.readOnly.of(true) : [history(), closeBrackets(), indentOnInput()],
        editorTheme,
        drawSelection(),
        highlightActiveLine(),
        bracketMatching(),
        highlightSelectionMatches(),
        options.of(configure()),
        flashField,
        searchField,
        language === 'python' ? [python(), pythonHighlight] : [],
        lsp ? lspExtensions(lsp, () => settings.keys) : [],
        keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...searchKeymap.filter((k) => !['Mod-f', 'F3', 'Mod-g'].includes(k.key!)), ...historyKeymap, ...completionKeymap, indentWithTab]),
        EditorView.updateListener.of((u) => {
          if (u.docChanged && !external) onchange?.(u.state.doc.toString())
          if (u.focusChanged && u.view.hasFocus) onfocus?.()
        }),
      ],
    })
    view = v
    return () => v.destroy()
  })

  $effect(() => {
    if (view && source !== view.state.doc.toString()) {
      external = true
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: source } })
      external = false
    }
  })

  $effect(() => {
    if (view && lsp) view.dispatch(setDiagnostics(view.state, toDiagnostics(diagnostics, view.state.doc)))
  })

  $effect(() => {
    if (view) view.dispatch({ effects: searchMarks(view, marks) })
  })

  $effect(() => {
    if (!view || !match || match.to > view.state.doc.length) return
    view.dispatch({ selection: { anchor: match.from, head: match.to }, effects: EditorView.scrollIntoView(match.from, { y: 'center' }) })
  })

  $effect(() => {
    if (!view || !reveal) return
    const line = view.state.doc.line(Math.min(reveal.line + 1, view.state.doc.lines))
    const anchor = Math.min(line.from + reveal.character, line.to)
    view.dispatch({ selection: { anchor }, effects: EditorView.scrollIntoView(anchor, { y: 'center' }) })
    flashLine(view, anchor)
  })

  $effect(() => {
    const ext = configure()
    view?.dispatch({ effects: options.reconfigure(ext) })
  })

  const focus = $derived(focused)
  $effect(() => {
    if (!view || readonly) return
    if (focus && !view.hasFocus) view.focus()
    if (!focus && view.hasFocus) view.contentDOM.blur()
  })
</script>

<div class="editor" class:readonly bind:this={host}></div>

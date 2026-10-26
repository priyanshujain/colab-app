import type { AuthState } from '../bindings/AuthState'
import type { Cell } from '../bindings/Cell'
import type { CellOutputs } from '../bindings/CellOutputs'
import type { CellPosition } from '../bindings/CellPosition'
import type { CommMessage } from '../bindings/CommMessage'
import type { Diagnostics } from '../bindings/Diagnostics'
import type { InputRequest } from '../bindings/InputRequest'
import type { KernelChoice } from '../bindings/KernelChoice'
import type { KernelStatusEvent } from '../bindings/KernelStatusEvent'
import type { NotebookChanged } from '../bindings/NotebookChanged'
import type { Api, SaveState } from './ipc'
import { applyEdit } from './edit'

const code = (id: string, source: string): Cell => ({ id, cell_type: 'code', source, metadata: {}, execution_count: null, outputs: [] })

const sample: Cell[] = [
  { id: 'intro', cell_type: 'markdown', metadata: {}, source: '# Training a tiny model\n\nRunning on a **mock** kernel that echoes the cell source. The loss is the mean squared error\n\n$$L(\\theta) = \\frac{1}{n}\\sum_{i=1}^{n} (y_i - f_\\theta(x_i))^2$$\n\nwith learning rate $\\eta = 10^{-3}$.\n\n| step | loss |\n| ---: | ---: |\n| 0 | 1.82 |\n| 100 | 0.41 |\n\n```python\nmodel = Model(dim=64)\n```' },
  code('imports', 'import os\nimport torch.nn as nn\nimport numpy as np\nfrom dataclasses import dataclass\n\nx = np.arange(10, dtype=np.float32)\nprint(x.mean(), len(x))'),
  code('model', '@dataclass\nclass Model:\n    """A linear model."""\n    dim: int = 64\n\n    def forward(self, x: np.ndarray) -> np.ndarray:\n        # scale then shift\n        return x * self.dim + 0.5\n\nm = Model(dim=8)\nm.forward(x)'),
  { ...code('loop', 'for i in range(3):\n    print(f"step {i}: {i ** 2 / 3:.2f}")'), execution_count: 4, metadata: { execution: { 'iopub.status.busy': '2026-10-16T09:00:00.100000Z', 'iopub.execute_input': '2026-10-16T09:00:00.102000Z', 'shell.execute_reply': '2026-10-16T09:01:03.600000Z', 'iopub.status.idle': '2026-10-16T09:01:03.600000Z' } } },
  code('bad', 'ratio = 1 / 0'),
  { id: 'video', cell_type: 'markdown', metadata: {}, source: 'Lecture recording:\n\n<iframe width="560" height="315" src="https://www.youtube.com/embed/kCc8FmEb1nY?start=90" title="Let\'s build GPT: from scratch, in code, spelled out." frameborder="0" allowfullscreen></iframe>\n\nSee [the docs](https://docs.python.org/3/) for more.' },
  code('ask', 'name = input("Your name: ")\nprint("hello", name)'),
]

const generated = (n: number): Cell[] =>
  Array.from({ length: n }, (_, i): Cell =>
    i % 5 === 0
      ? { id: `c${i}`, cell_type: 'markdown', source: `## Section ${i / 5}\n\nNotes for cell ${i}.`, metadata: {} }
      : code(`c${i}`, Array.from({ length: (i % 4) + 1 }, (_, j) => `v${i}_${j} = ${i} * ${j}`).join('\n')),
  )

const library = '"""Mock numpy."""\n\n\ndef arange(start, stop=None, step=1, dtype=None):\n    """Return evenly spaced values within a given interval."""\n    ...\n\n\nclass ndarray:\n    def mean(self, axis=None):\n        ...\n'

export function mockApi(): Api {
  const params = new URLSearchParams(location.search)
  const count = Number(params.get('cells'))
  let cells = count ? generated(count) : sample
  let path = 'sample.ipynb'
  let execution = 0
  let choice: KernelChoice | null = null
  let started = false
  let auth: AuthState = { state: 'signed_out' }
  let reply: ((v: string) => void) | null = null
  const outputs: ((e: CellOutputs) => void)[] = []
  const changed: ((e: NotebookChanged) => void)[] = []
  const statuses: ((e: KernelStatusEvent) => void)[] = []
  const auths: ((e: AuthState) => void)[] = []
  const diags: ((e: Diagnostics) => void)[] = []
  const inputs: ((e: InputRequest) => void)[] = []
  const saves: ((e: SaveState) => void)[] = []
  let saving: ReturnType<typeof setTimeout> | undefined
  const save = (dirty: boolean) => saves.forEach((cb) => cb({ path, dirty }))
  const status = (s: KernelStatusEvent['status']) => statuses.forEach((cb) => cb({ path, status: s, choice }))
  const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms))
  const sub = <T>(list: T[]) => async (cb: T) => (list.push(cb), () => void list.splice(list.indexOf(cb), 1))
  const word = (p: CellPosition) => {
    const line = cells.find((c) => c.id === p.cell_id)?.source.split('\n')[p.line] ?? ''
    return (line.slice(0, p.character).match(/\w*$/)![0] + line.slice(p.character).match(/^\w*/)![0]) || null
  }
  const lint = () => cells.filter((c) => c.cell_type === 'code').forEach((c) => {
    const all = cells.map((x) => x.source).join('\n')
    const diagnostics = [...c.source.matchAll(/^import (\w+)$/gm)]
      .filter((m) => all.split(new RegExp(`\\b${m[1]}\\b`)).length === 2)
      .map((m) => {
        const line = c.source.slice(0, m.index).split('\n').length - 1
        return { range: { start: { line, character: 7 }, end: { line, character: 7 + m[1].length } }, severity: 2, source: 'Ruff', code: 'F401', message: `\`${m[1]}\` imported but unused` }
      })
      .concat([...c.source.matchAll(/^import (\w+)\.(\w+) as \2$/gm)].map((m) => {
        const line = c.source.slice(0, m.index).split('\n').length - 1
        return { range: { start: { line, character: 7 }, end: { line, character: m[0].length } }, severity: 2, source: 'Ruff', code: 'PLR0402', message: `Use \`from ${m[1]} import ${m[2]}\` in lieu of alias` }
      }))
    diags.forEach((cb) => cb({ path, cell_id: c.id, diagnostics }))
  })
  const emitCells = () => changed.forEach((cb) => cb({ path, notebook: { nbformat: 4, nbformat_minor: 5, metadata: {}, cells } }))
  const format = (id: string) => {
    cells = cells.map((c) => (c.id === id && c.cell_type === 'code' ? { ...c, source: c.source.split('\n').map((l) => l.trimEnd().replace(/,(?=\S)/g, ', ')).join('\n').trim() } : c))
  }
  const organize = (id: string) => {
    cells = cells.map((c) => {
      if (c.id !== id || c.cell_type !== 'code') return c
      const lines = c.source.split('\n')
      const n = lines.findIndex((l) => !/^(import|from) /.test(l))
      const end = n < 0 ? lines.length : n
      return { ...c, source: [...lines.slice(0, end).sort(), ...lines.slice(end)].join('\n') }
    })
  }
  let actions: { cell: string; line: number; text: string }[] = []
  const run = async (cell_id: string) => {
    const cell = cells.find((c) => c.id === cell_id)
    if (!cell || cell.cell_type !== 'code') return
    const emit = (e: Omit<CellOutputs, 'path' | 'cell_id'>) => outputs.forEach((cb) => cb({ path, cell_id, ...e }))
    if (!started) {
      choice ??= { kind: 'local' }
      status('connecting')
      await sleep(400)
      started = true
    }
    status('busy')
    const busy = new Date().toISOString()
    emit({ outputs: [], execution_count: null, execution: { 'iopub.status.busy': busy }, running: true })
    await sleep(300)
    let text = cell.source + '\n'
    const ask = cell.source.match(/input\("([^"]*)"\)/)
    if (ask) {
      const value = await new Promise<string>((r) => ((reply = r), inputs.forEach((cb) => cb({ path, cell_id, prompt: ask[1], password: false }))))
      text = `${ask[1]}${value}\nhello ${value}\n`
    }
    const done = new Date().toISOString()
    const error = /raise|1 \/ 0/.test(cell.source)
    const timing = { 'iopub.status.busy': busy, 'iopub.execute_input': busy, 'shell.execute_reply': done, 'iopub.status.idle': done }
    emit({ outputs: error ? [{ output_type: 'error', ename: 'ZeroDivisionError', evalue: 'division by zero', traceback: [] }] : [{ output_type: 'stream', name: 'stdout', text }], execution_count: ++execution, execution: timing, running: false })
    status('idle')
  }
  setTimeout(lint, 200)
  return {
    openNotebook: async (p) => ((path = p), { path, notebook: { nbformat: 4, nbformat_minor: 5, metadata: {}, cells } }),
    editCell: async (_, edit) => {
      cells = applyEdit(cells, edit)
      if (edit.kind === 'focus') return
      lint()
      save(true)
      clearTimeout(saving)
      saving = setTimeout(() => save(false), 800)
    },
    runCell: (_, id) => run(id),
    runAll: async () => { for (const c of cells) await run(c.id) },
    interrupt: async () => status('idle'),
    restart: async () => { execution = 0; if (started) status('idle') },
    selectKernel: async (_, c) => {
      choice = c
      started = c.kind === 'colab'
      if (!started) return status('disconnected')
      status('connecting')
      await sleep(500)
      status('idle')
    },
    sendComm: async (_: CommMessage) => {},
    signOut: async () => { auth = { state: 'signed_out' }; auths.forEach((cb) => cb(auth)) },
    signIn: async (clientId) => {
      await sleep(600)
      auth = params.get('auth') === 'rejected' && !clientId ? { state: 'client_rejected', message: 'Google rejected the OAuth client (invalid_client).' } : { state: 'signed_in', email: 'you@gmail.com' }
      auths.forEach((cb) => cb(auth))
      return auth
    },
    authState: async () => auth,
    colabInfo: async () => ({ balance: 87.42, hourly_rate: choice?.kind === 'colab' ? 1.76 : 0, eligible_gpus: ['T4', 'L4', 'A100'], eligible_tpus: ['V5E1', 'V6E1'] }),
    listRuntimes: async () => [],
    detach: async () => { started = false; choice = null; status('disconnected') },
    release: async () => { started = false; choice = null; status('disconnected') },
    checkLink: async (url) => {
      await sleep(400)
      if (!/colab\.research\.google\.com\/(drive|github)\//.test(url)) throw new Error(`not a Colab notebook link: ${url}`)
      if (/private/.test(url)) throw new Error('the notebook is not shared publicly; in Colab use Share and set General access to Anyone with the link')
      if (/missing/.test(url)) throw new Error('the notebook does not exist; check the link')
      return 'CLIP Guided Diffusion HQ 256x256.ipynb'
    },
    cloneLink: async (_, dest) => (await sleep(300), dest),
    prepareRename: async (p) => {
      const w = word(p)
      return w && !/^\d/.test(w) ? { defaultBehavior: true } : null
    },
    renameSymbol: async (p, name) => {
      const w = word(p)
      if (!w) throw new Error('nothing to rename here')
      cells = cells.map((c) => (c.cell_type === 'code' ? { ...c, source: c.source.replace(new RegExp(`\\b${w}\\b`, 'g'), name) } : c))
      emitCells()
    },
    openUrl: async (url) => void window.open(url, '_blank'),
    version: async () => '0.1.0',
    completion: async (p) => {
      const names = new Set(cells.flatMap((c) => [...c.source.matchAll(/\b([A-Za-z_]\w*)\s*=|\b(?:def|class) (\w+)/g)].map((m) => m[1] ?? m[2])))
      return { isIncomplete: false, items: [
        ...[...names].map((label) => ({ label, kind: 6, detail: 'variable' })),
        { label: 'arange', kind: 3, detail: '(start, stop=None, step=1, dtype=None) -> ndarray', documentation: { kind: 'markdown', value: 'Return evenly spaced values within a given interval.\n\n```python\nnp.arange(3)\n```' } },
        { label: 'mean', kind: 2, detail: '(axis=None) -> float', documentation: { kind: 'markdown', value: 'Compute the arithmetic mean along the specified axis.' } },
      ] }
    },
    hover: async (p) => {
      const w = word(p)
      return w && { contents: { kind: 'markdown', value: `\`\`\`python\n${w}: ${w === 'np' ? 'Module[numpy]' : 'int'}\n\`\`\`\n---\nMock hover for **${w}**.` } }
    },
    definition: async (p) => {
      const w = word(p)
      if (w === 'np' || w === 'arange') return [{ uri: 'file:///mock/site-packages/numpy/__init__.py', range: { start: { line: 3, character: 4 }, end: { line: 3, character: 10 } } }]
      for (const c of cells) {
        const lines = c.source.split('\n')
        const line = lines.findIndex((l) => new RegExp(`^\\s*(def |class )?${w}\\b`).test(l))
        if (line >= 0) return [{ uri: `vscode-notebook-cell://${path}#${c.id}`, range: { start: { line, character: lines[line].indexOf(w!) }, end: { line, character: lines[line].indexOf(w!) + w!.length } } }]
      }
      return null
    },
    formatCell: async (_, id) => { organize(id); format(id); emitCells() },
    formatNotebook: async () => { cells.forEach((c) => (organize(c.id), format(c.id))); emitCells() },
    organizeImports: async (_, id) => { cells.forEach((c) => (id ?? c.id) === c.id && organize(c.id)); emitCells() },
    codeActions: async (p) => {
      const line = cells.find((c) => c.id === p.cell_id)?.source.split('\n')[p.line] ?? ''
      const alias = line.match(/^import (\w+)\.(\w+) as \2$/)
      actions = alias ? [{ cell: p.cell_id, line: p.line, text: `from ${alias[1]} import ${alias[2]}` }, { cell: p.cell_id, line: p.line, text: `${line}  # noqa: PLR0402` }] : []
      return actions.map((a, id) => ({ id, title: id ? 'Ruff (PLR0402): Disable for this line' : `Ruff (PLR0402): Replace with \`${a.text}\`` }))
    },
    applyCodeAction: async (_, id) => {
      const a = actions[id]
      cells = cells.map((c) => (c.id === a.cell ? { ...c, source: c.source.split('\n').map((l, i) => (i === a.line ? a.text : l)).join('\n') } : c))
      emitCells()
      lint()
    },
    renameNotebook: async (p, name) => {
      if (/[/\\]/.test(name)) throw new Error('Name cannot contain a slash')
      return (path = p.replace(/[^/]*$/, name))
    },
    listMonospaceFonts: async () => ['Fira Code', 'JetBrains Mono', 'Menlo', 'Monaco', 'SF Mono'],
    listFonts: async () => ['Avenir Next', 'Georgia', 'Helvetica Neue', 'Inter', 'JetBrains Mono', 'Menlo', 'Monaco', 'Times New Roman'],
    readFile: async (p) => (p.includes('numpy') ? library : `# ${p}\n`),
    sendInput: async (_, value) => { reply?.(value); reply = null },
    fileMenu: async () => {},
    pickNotebook: async () => prompt('Notebook path', 'sample.ipynb'),
    pickSave: async (d) => prompt('Save to', d),
    onCellOutputs: sub(outputs),
    onKernelStatus: sub(statuses),
    onNotebookChanged: sub(changed),
    onAuthState: sub(auths),
    onComm: async () => () => {},
    onDiagnostics: sub(diags),
    onInputRequest: sub(inputs),
    onSaveState: sub(saves),
    onAgentFocus: async () => () => {},
    windowStartup: async () => null,
    kernelState: async (path) => ({ path, status: 'disconnected', choice: null }),
    onMenu: async () => () => {},
  }
}

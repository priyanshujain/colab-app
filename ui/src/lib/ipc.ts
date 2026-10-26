import type { AgentFocus } from '../bindings/AgentFocus'
import type { AuthState } from '../bindings/AuthState'
import type { CellEdit } from '../bindings/CellEdit'
import type { CellOutputs } from '../bindings/CellOutputs'
import type { CellPosition } from '../bindings/CellPosition'
import type { CodeActionItem } from '../bindings/CodeActionItem'
import type { ColabInfo } from '../bindings/ColabInfo'
import type { CommMessage } from '../bindings/CommMessage'
import type { Diagnostics } from '../bindings/Diagnostics'
import type { InputRequest } from '../bindings/InputRequest'
import type { KernelChoice } from '../bindings/KernelChoice'
import type { KernelStatusEvent } from '../bindings/KernelStatusEvent'
import type { NotebookChanged } from '../bindings/NotebookChanged'
import type { OpenedNotebook } from '../bindings/OpenedNotebook'
import type { Runtime } from '../bindings/Runtime'
import { mockApi } from './mock'

type Unlisten = () => void
export type { SaveState } from "../bindings/SaveState"
import type { SaveState } from "../bindings/SaveState"

export interface Api {
  openNotebook(path: string): Promise<OpenedNotebook | null>
  windowStartup(): Promise<string | null>
  editCell(path: string, edit: CellEdit): Promise<void>
  runCell(path: string, cellId: string): Promise<void>
  runAll(path: string): Promise<void>
  interrupt(path: string): Promise<void>
  restart(path: string): Promise<void>
  selectKernel(path: string, choice: KernelChoice): Promise<void>
  sendComm(message: CommMessage): Promise<void>
  signIn(clientId: string | null, clientSecret: string | null): Promise<AuthState>
  signOut(): Promise<void>
  authState(): Promise<AuthState>
  colabInfo(): Promise<ColabInfo>
  listRuntimes(): Promise<Runtime[]>
  detach(path: string): Promise<void>
  release(path: string): Promise<void>
  cloneLink(url: string, dest: string): Promise<string>
  checkLink(url: string): Promise<string>
  prepareRename(pos: CellPosition): Promise<unknown>
  renameSymbol(pos: CellPosition, newName: string): Promise<void>
  openUrl(url: string): Promise<void>
  version(): Promise<string>
  completion(pos: CellPosition): Promise<unknown>
  hover(pos: CellPosition): Promise<unknown>
  definition(pos: CellPosition): Promise<unknown>
  formatCell(path: string, cellId: string): Promise<void>
  formatNotebook(path: string): Promise<void>
  organizeImports(path: string, cellId: string | null): Promise<void>
  codeActions(pos: CellPosition): Promise<CodeActionItem[]>
  applyCodeAction(path: string, id: number): Promise<void>
  readFile(path: string): Promise<string>
  listMonospaceFonts(): Promise<string[]>
  listFonts(): Promise<string[]>
  renameNotebook(path: string, newName: string): Promise<string>
  sendInput(path: string, value: string): Promise<void>
  fileMenu(path: string): Promise<void>
  listSecrets(): Promise<Record<string, string>>
  setSecret(name: string, value: string): Promise<void>
  deleteSecret(name: string): Promise<void>
  pickNotebook(): Promise<string | null>
  pickSave(defaultPath: string): Promise<string | null>
  onCellOutputs(cb: (e: CellOutputs) => void): Promise<Unlisten>
  onKernelStatus(cb: (e: KernelStatusEvent) => void): Promise<Unlisten>
  onNotebookChanged(cb: (e: NotebookChanged) => void): Promise<Unlisten>
  onAuthState(cb: (e: AuthState) => void): Promise<Unlisten>
  onComm(cb: (e: CommMessage) => void): Promise<Unlisten>
  onDiagnostics(cb: (e: Diagnostics) => void): Promise<Unlisten>
  onInputRequest(cb: (e: InputRequest) => void): Promise<Unlisten>
  onSaveState(cb: (e: SaveState) => void): Promise<Unlisten>
  onAgentFocus(cb: (e: AgentFocus) => void): Promise<Unlisten>
  kernelState(path: string): Promise<KernelStatusEvent>
  diagnostics(path: string): Promise<Diagnostics[]>
  onMenu(cb: (id: string) => void): Promise<Unlisten>
}

const filters = [{ name: 'Notebook', extensions: ['ipynb'] }]

async function tauriApi(): Promise<Api> {
  const { invoke } = await import('@tauri-apps/api/core')
  const { listen } = await import('@tauri-apps/api/event')
  const dialog = await import('@tauri-apps/plugin-dialog')
  const on = <T>(name: string) => (cb: (e: T) => void) => listen<T>(name, (e) => cb(e.payload))
  return {
    openNotebook: (path) => invoke('open_notebook', { path }),
    windowStartup: () => invoke('window_startup'),
    editCell: (path, edit) => invoke('edit_cell', { path, edit }),
    runCell: (path, cellId) => invoke('run_cell', { path, cellId }),
    runAll: (path) => invoke('run_all', { path }),
    interrupt: (path) => invoke('interrupt', { path }),
    restart: (path) => invoke('restart', { path }),
    selectKernel: (path, choice) => invoke('select_kernel', { path, choice }),
    sendComm: (message) => invoke('send_comm', { message }),
    signIn: (clientId, clientSecret) => invoke('sign_in', { clientId, clientSecret }),
    signOut: () => invoke('sign_out'),
    authState: () => invoke('auth_state'),
    colabInfo: () => invoke('colab_info'),
    listRuntimes: () => invoke('list_runtimes'),
    detach: (path) => invoke('detach', { path }),
    release: (path) => invoke('release', { path }),
    cloneLink: (url, dest) => invoke('clone_link', { url, dest }),
    checkLink: (url) => invoke('check_link', { url }),
    prepareRename: (pos) => invoke('prepare_rename', { pos }),
    renameSymbol: (pos, newName) => invoke('rename_symbol', { pos, newName }),
    openUrl: (url) => invoke('open_url', { url }),
    version: async () => (await import('@tauri-apps/api/app')).getVersion(),
    completion: (pos) => invoke('completion', { pos }),
    hover: (pos) => invoke('hover', { pos }),
    definition: (pos) => invoke('definition', { pos }),
    formatCell: (path, cellId) => invoke('format_cell', { path, cellId }),
    formatNotebook: (path) => invoke('format_notebook', { path }),
    organizeImports: (path, cellId) => invoke('organize_imports', { path, cellId }),
    codeActions: (pos) => invoke('code_actions', { pos }),
    applyCodeAction: (path, id) => invoke('apply_code_action', { path, id }),
    readFile: (path) => invoke('read_file', { path }),
    listMonospaceFonts: once(() => invoke<string[]>('list_monospace_fonts')),
    listFonts: once(() => invoke<string[]>('list_fonts')),
    renameNotebook: (path, newName) => invoke('rename_notebook', { path, newName }),
    sendInput: (path, value) => invoke('send_input', { path, value }),
    fileMenu: (path) => invoke('file_menu', { path }),
    listSecrets: () => invoke('list_secrets'),
    setSecret: (name, value) => invoke('set_secret', { name, value }),
    deleteSecret: (name) => invoke('delete_secret', { name }),
    pickNotebook: async () => (await dialog.open({ filters, multiple: false, directory: false })) as string | null,
    pickSave: (defaultPath) => dialog.save({ defaultPath, filters }),
    onCellOutputs: on('cell-outputs'),
    onKernelStatus: on('kernel-status'),
    onNotebookChanged: on('notebook-changed'),
    onAuthState: on('auth-state'),
    onComm: on('comm'),
    onDiagnostics: on('diagnostics'),
    onInputRequest: on('input-request'),
    onSaveState: on('save-state'),
    onAgentFocus: on('agent-focus'),
    kernelState: (path) => invoke('kernel_state', { path }),
    diagnostics: (path) => invoke('diagnostics', { path }),
    onMenu: on('menu'),
  }
}

export const inTauri = '__TAURI_INTERNALS__' in window
export const api: Promise<Api> = inTauri ? tauriApi() : Promise.resolve(mockApi())

function once<T>(f: () => Promise<T>): () => Promise<T> {
  let p: Promise<T> | undefined
  return () => (p ??= f().catch((e) => ((p = undefined), Promise.reject(e))))
}

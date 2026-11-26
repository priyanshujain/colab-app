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
import type { Machine } from '../bindings/Machine'
import type { NotebookChanged } from '../bindings/NotebookChanged'
import type { OpenedNotebook } from '../bindings/OpenedNotebook'
import type { Runtime } from '../bindings/Runtime'
import { mockApi } from './mock'

type Unlisten = () => void
export type Host = { label: string; address: string }
export type { TailscaleStatus } from '../bindings/TailscaleStatus'
import type { TailscaleStatus } from '../bindings/TailscaleStatus'
export type { HostStatus } from '../bindings/HostStatus'
import type { HostStatus } from '../bindings/HostStatus'
export type Listing = { path: string; entries: { name: string; dir: boolean }[] }
export type { SaveState } from "../bindings/SaveState"
import type { SaveState } from "../bindings/SaveState"

export interface Api {
  openNotebook(path: string): Promise<OpenedNotebook | null>
  windowStartup(): Promise<string | null>
  windowHost(): Promise<string | null>
  listDir(path: string): Promise<Listing>
  listHosts(): Promise<Host[]>
  addHost(label: string, address: string): Promise<Host[]>
  removeHost(label: string): Promise<Host[]>
  onServerOutdated(cb: (notice: string | null) => void): Promise<Unlisten>
  tailscaleStatus(): Promise<TailscaleStatus>
  tailscaleEnable(hostname?: string): Promise<TailscaleStatus>
  tailscaleDisable(): Promise<TailscaleStatus>
  tailscaleLogout(): Promise<TailscaleStatus>
  onTailscaleStatus(cb: (e: TailscaleStatus) => void): Promise<Unlisten>
  hostStatus(): Promise<HostStatus[]>
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
  openAgentNotebooks(): Promise<boolean>
  setOpenAgentNotebooks(on: boolean): Promise<void>
  serverStatus(): Promise<string>
  stopServer(restart: boolean): Promise<void>
  startServer(): Promise<void>
  machine(): Promise<Machine>
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
  runtimeStarted(path: string): Promise<string | null>
  diagnostics(path: string): Promise<Diagnostics[]>
  onMenu(cb: (id: string) => void): Promise<Unlisten>
}

const filters = [{ name: 'Notebook', extensions: ['ipynb'] }]

async function tauriApi(): Promise<Api> {
  const { invoke } = await import('@tauri-apps/api/core')
  const dialog = await import('@tauri-apps/plugin-dialog')
  const current = (await import('@tauri-apps/api/webviewWindow')).getCurrentWebviewWindow()
  const on = <T>(name: string) => (cb: (e: T) => void) => current.listen<T>(name, (e) => cb(e.payload))
  return {
    openNotebook: (path) => invoke('call', { method: 'open_notebook', params: { path } }),
    windowStartup: () => invoke('window_startup'),
    windowHost: () => invoke('window_host'),
    listDir: (path) => invoke('call', { method: 'list_dir', params: { path } }),
    listHosts: () => invoke('list_hosts'),
    addHost: (label, address) => invoke('add_host', { label, address }),
    removeHost: (label) => invoke('remove_host', { label }),
    onServerOutdated: on('server-outdated'),
    tailscaleStatus: () => invoke('call', { method: 'tailscale_status' }),
    tailscaleEnable: (hostname) => invoke('call', { method: 'tailscale_enable', params: { hostname } }),
    tailscaleDisable: () => invoke('call', { method: 'tailscale_disable' }),
    tailscaleLogout: () => invoke('call', { method: 'tailscale_logout' }),
    onTailscaleStatus: on('tailscale-status'),
    hostStatus: () => invoke('call', { method: 'host_status' }),
    editCell: (path, edit) => invoke('call', { method: 'edit_cell', params: { path, edit } }),
    runCell: (path, cellId) => invoke('call', { method: 'run_cell', params: { path, cellId } }),
    runAll: (path) => invoke('call', { method: 'run_all', params: { path } }),
    interrupt: (path) => invoke('call', { method: 'interrupt', params: { path } }),
    restart: (path) => invoke('call', { method: 'restart', params: { path } }),
    selectKernel: (path, choice) => invoke('call', { method: 'select_kernel', params: { path, choice } }),
    sendComm: (message) => invoke('call', { method: 'send_comm', params: { message } }),
    signIn: (clientId, clientSecret) => invoke('sign_in', { clientId, clientSecret }),
    signOut: () => invoke('call', { method: 'sign_out' }),
    authState: () => invoke('call', { method: 'auth_state' }),
    colabInfo: () => invoke('call', { method: 'colab_info' }),
    listRuntimes: () => invoke('call', { method: 'list_runtimes' }),
    detach: (path) => invoke('call', { method: 'detach', params: { path } }),
    release: (path) => invoke('call', { method: 'release', params: { path } }),
    cloneLink: (url, dest) => invoke('call', { method: 'clone_link', params: { url, dest } }),
    checkLink: (url) => invoke('check_link', { url }),
    prepareRename: (pos) => invoke('call', { method: 'prepare_rename', params: { pos } }),
    renameSymbol: (pos, newName) => invoke('call', { method: 'rename_symbol', params: { pos, newName } }),
    openUrl: (url) => invoke('open_url', { url }),
    version: async () => (await import('@tauri-apps/api/app')).getVersion(),
    openAgentNotebooks: () => invoke('open_agent_notebooks'),
    setOpenAgentNotebooks: (on) => invoke('set_open_agent_notebooks', { on }),
    serverStatus: () => invoke('server_status'),
    stopServer: (restart) => invoke('stop_server', { restart }),
    startServer: () => invoke('start_server'),
    machine: () => invoke('call', { method: 'machine' }),
    completion: (pos) => invoke('call', { method: 'completion', params: { pos } }),
    hover: (pos) => invoke('call', { method: 'hover', params: { pos } }),
    definition: (pos) => invoke('call', { method: 'definition', params: { pos } }),
    formatCell: (path, cellId) => invoke('call', { method: 'format_cell', params: { path, cellId } }),
    formatNotebook: (path) => invoke('call', { method: 'format_notebook', params: { path } }),
    organizeImports: (path, cellId) => invoke('call', { method: 'organize_imports', params: { path, cellId } }),
    codeActions: (pos) => invoke('call', { method: 'code_actions', params: { pos } }),
    applyCodeAction: (path, id) => invoke('call', { method: 'apply_code_action', params: { path, id } }),
    readFile: (path) => invoke('call', { method: 'read_file', params: { path } }),
    listMonospaceFonts: once(() => invoke<string[]>('list_monospace_fonts')),
    listFonts: once(() => invoke<string[]>('list_fonts')),
    renameNotebook: (path, newName) => invoke('call', { method: 'rename_notebook', params: { path, newName } }),
    sendInput: (path, value) => invoke('call', { method: 'send_input', params: { path, value } }),
    fileMenu: (path) => invoke('file_menu', { path }),
    listSecrets: () => invoke('call', { method: 'list_secrets' }),
    setSecret: (name, value) => invoke('call', { method: 'set_secret', params: { name, value } }),
    deleteSecret: (name) => invoke('call', { method: 'delete_secret', params: { name } }),
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
    kernelState: (path) => invoke('call', { method: 'kernel_state', params: { path } }),
    runtimeStarted: (path) => invoke('call', { method: 'runtime_started', params: { path } }),
    diagnostics: (path) => invoke('call', { method: 'diagnostics', params: { path } }),
    onMenu: on('menu'),
  }
}

export const inTauri = '__TAURI_INTERNALS__' in window
export const api: Promise<Api> = inTauri ? tauriApi() : Promise.resolve(mockApi())

function once<T>(f: () => Promise<T>): () => Promise<T> {
  let p: Promise<T> | undefined
  return () => (p ??= f().catch((e) => ((p = undefined), Promise.reject(e))))
}

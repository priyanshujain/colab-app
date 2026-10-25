import './jquery'
import './jquery-ui'
import { HTMLManager } from '@jupyter-widgets/html-manager/lib/htmlmanager'
import type { WidgetModel } from '@jupyter-widgets/base'
import '@jupyter-widgets/controls/css/widgets.css'
import type { CommMessage } from '../../bindings/CommMessage'
import { sendComm } from './comm'

type Handler = (msg: unknown) => void
type Buffer = ArrayBuffer | ArrayBufferView
type Callbacks = { iopub?: { status?: Handler } }

const toBase64 = (b: Buffer) => {
  const bytes = b instanceof ArrayBuffer ? new Uint8Array(b) : new Uint8Array(b.buffer, b.byteOffset, b.byteLength)
  return btoa(Array.from(bytes, c => String.fromCharCode(c)).join(''))
}
const fromBase64 = (s: string) => new DataView(Uint8Array.from(atob(s), c => c.charCodeAt(0)).buffer)

class Comm {
  onMsg: Handler = () => {}
  onClose: Handler = () => {}
  constructor(readonly comm_id: string, readonly target_name: string, private path: string) {}
  private post(msg_type: string, data: unknown, metadata?: Record<string, unknown>, callbacks?: Callbacks, buffers: Buffer[] = []) {
    void sendComm({ path: this.path, comm_id: this.comm_id, msg_type, target_name: this.target_name, buffers: buffers.map(toBase64), data: data ?? {}, metadata: metadata ?? {} })
      .finally(() => callbacks?.iopub?.status?.({ content: { execution_state: 'idle' } }))
    return this.comm_id
  }
  open() { return this.comm_id }
  send(data: unknown, callbacks?: Callbacks, metadata?: Record<string, unknown>, buffers?: Buffer[]) { return this.post('comm_msg', data, metadata, callbacks, buffers) }
  close(data?: unknown, callbacks?: Callbacks) { return this.post('comm_close', data, undefined, callbacks) }
  on_msg(f: Handler) { this.onMsg = f }
  on_close(f: Handler) { this.onClose = f }
}

class Manager extends HTMLManager {
  comms = new Map<string, Comm>()
  waiting = new Map<string, (() => void)[]>()
  constructor(private path: string) { super() }
  _create_comm(target: string, id: string) {
    const comm = new Comm(id, target, this.path)
    this.comms.set(id, comm)
    return Promise.resolve(comm)
  }
  register_model(id: string, model: Promise<WidgetModel>) {
    super.register_model(id, model)
    this.waiting.get(id)?.forEach(f => f())
    this.waiting.delete(id)
  }
  model(id: string): Promise<WidgetModel> {
    return this.get_model(id) ?? new Promise(resolve => this.waiting.set(id, [...this.waiting.get(id) ?? [], () => resolve(this.get_model(id)!)]))
  }
}

const managers = new Map<string, Manager>()
export const manager = (path: string) => managers.get(path) ?? managers.set(path, new Manager(path)).get(path)!

export async function handleComm(m: CommMessage) {
  const mgr = manager(m.path)
  const target_name = m.target_name ?? 'jupyter.widget'
  const msg = { content: { comm_id: m.comm_id, target_name, data: m.data }, metadata: m.metadata, buffers: m.buffers.map(fromBase64) }
  if (m.msg_type === 'comm_open') {
    if (target_name !== 'jupyter.widget') return
    const comm = new Comm(m.comm_id, target_name, m.path)
    mgr.comms.set(m.comm_id, comm)
    await mgr.handle_comm_open(comm as never, msg as never)
  } else {
    const comm = mgr.comms.get(m.comm_id)
    if (m.msg_type === 'comm_close') mgr.comms.delete(m.comm_id)
    comm?.[m.msg_type === 'comm_close' ? 'onClose' : 'onMsg'](msg)
  }
}

const restored = new Map<string, Promise<unknown>>()
export const restoreState = (path: string, state: unknown) =>
  restored.get(path) ?? restored.set(path, manager(path).set_state(state as never).catch(console.error)).get(path)!

export async function renderWidget(path: string, modelId: string, el: HTMLElement) {
  const mgr = manager(path)
  const view = await mgr.create_view(await mgr.model(modelId), {})
  await mgr.display_view(view, el)
  return () => { try { view.remove() } catch { view.model.off(null, null, view) } }
}

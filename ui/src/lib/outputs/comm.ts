import type { CommMessage } from '../../bindings/CommMessage'

const tauri = typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

export async function sendComm(message: CommMessage) {
  if (!tauri) return
  const { invoke } = await import('@tauri-apps/api/core')
  await invoke('send_comm', { message })
}

export async function onComm(handler: (message: CommMessage) => void): Promise<() => void> {
  if (!tauri) return () => {}
  const { listen } = await import('@tauri-apps/api/event')
  return listen<CommMessage>('comm', e => handler(e.payload))
}

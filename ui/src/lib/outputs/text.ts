import { AnsiUp } from 'ansi_up'

export const MAX_LINES = 2000
export const MAX_CHARS = 1_000_000

export const collapseCarriageReturns = (text: string) =>
  text.split('\n').map(line => {
    const s = line.replace(/\r+$/, '')
    return s.slice(s.lastIndexOf('\r') + 1)
  }).join('\n')

export function truncate(text: string): string | null {
  const lines = text.slice(0, MAX_CHARS).split('\n')
  return text.length > MAX_CHARS || lines.length > MAX_LINES ? lines.slice(0, MAX_LINES).join('\n') : null
}

export const ansiToHtml = (text: string) => {
  const ansi = new AnsiUp()
  ansi.use_classes = true
  return ansi.ansi_to_html(text)
}

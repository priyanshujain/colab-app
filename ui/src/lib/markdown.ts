import katex from 'katex'
import { Marked, type TokenizerAndRendererExtension } from 'marked'
import { escape, highlightPython } from './highlight'

const math = (name: string, display: boolean, re: RegExp): TokenizerAndRendererExtension => ({
  name,
  level: display ? 'block' : 'inline',
  start: (src) => src.indexOf(display ? '$$' : '$'),
  tokenizer(src) {
    const m = re.exec(src)
    return m ? { type: name, raw: m[0], text: m[1].trim() } : undefined
  },
  renderer: (token) => katex.renderToString(token.text, { displayMode: display, throwOnError: false }),
})

const md = new Marked({
  extensions: [math('blockMath', true, /^\$\$([\s\S]+?)\$\$/), math('inlineMath', false, /^\$((?:\\\$|[^$\n])+?)\$/)],
  renderer: {
    code({ text, lang }) {
      const body = !lang || /^(py|python|ipython3?)$/.test(lang) ? highlightPython(text) : escape(text)
      return `<pre class="code"><code>${body}</code></pre>`
    },
  },
})

const seconds = (t: string | null) => {
  const m = t?.match(/^(?:(\d+)h)?(?:(\d+)m)?(?:(\d+)s?)?$/)
  return m ? Number(m[1] ?? 0) * 3600 + Number(m[2] ?? 0) * 60 + Number(m[3] ?? 0) : 0
}

export function youtube(src: string) {
  let url: URL
  try {
    url = new URL(src.replace(/&amp;/g, '&'), 'https://invalid.local')
  } catch {
    return null
  }
  const host = url.hostname.replace(/^(www|m)\./, '')
  const id = host === 'youtu.be' ? url.pathname.slice(1)
    : /^youtube(-nocookie)?\.com$/.test(host) ? (url.pathname.match(/^\/(?:embed|shorts|live)\/([\w-]+)/)?.[1] ?? (url.pathname === '/watch' ? url.searchParams.get('v') : null))
    : null
  if (!id || !/^[\w-]{6,}$/.test(id)) return null
  const start = seconds(url.searchParams.get('start') ?? url.searchParams.get('t'))
  return { id, watch: `https://www.youtube.com/watch?v=${id}${start ? `&t=${start}s` : ''}` }
}

const attr = (tag: string, name: string) => {
  const m = tag.match(new RegExp(`\\s${name}=(?:"([^"]*)"|'([^']*)')`, 'i'))
  return m ? (m[1] ?? m[2]) : undefined
}

const card = (src: string, title: string | undefined, width: string | undefined, fallback: string) => {
  const v = youtube(src)
  if (!v) return fallback
  const w = Number(width) > 0 ? ` style="width: min(${Number(width)}px, 100%)"` : ''
  return `<a class="yt-card" href="${v.watch}" title="Watch on YouTube"${w}><img src="https://i.ytimg.com/vi/${v.id}/hqdefault.jpg" alt="" loading="lazy"><span class="yt-play"></span><span class="yt-title">${title && title !== 'YouTube video player' ? title : 'Watch on YouTube'}</span></a>`
}

export const renderMarkdown = (src: string) =>
  (md.parse(src, { async: false }) as string)
    .replace(/<iframe\b[^>]*>[\s\S]*?<\/iframe>|<iframe\b[^>]*\/?>/gi, (tag) => card(attr(tag, 'src') ?? '', attr(tag, 'title'), attr(tag, 'width'), tag))
    .replace(/<p><a href="([^"]+)"[^>]*>([^<]*)<\/a><\/p>/g, (p, href, text) => card(href, text && text !== href ? text : undefined, undefined, p))

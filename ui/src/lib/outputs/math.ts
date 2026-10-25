import katex from 'katex'
import 'katex/dist/katex.min.css'
import { marked } from 'marked'

const tex = (src: string, displayMode: boolean) => katex.renderToString(src, { displayMode, throwOnError: false })

export const renderMath = (text: string) =>
  text.replace(/\$\$([\s\S]+?)\$\$|\\\[([\s\S]+?)\\\]/g, (_, a, b) => tex(a ?? b, true))
    .replace(/\$([^$\n]+?)\$|\\\(([\s\S]+?)\\\)/g, (_, a, b) => tex(a ?? b, false))

export const renderMarkdown = (text: string) => marked.parse(renderMath(text), { async: false })

export const WIDGET = 'application/vnd.jupyter.widget-view+json'
export const PLOTLY = 'application/vnd.plotly.v1+json'
const VEGALITE = /^application\/vnd\.vegalite\.v(\d+)(\+json|\.json)$/
const ORDER = [WIDGET, PLOTLY, 'vegalite', 'application/javascript', 'text/html', 'image/svg+xml', 'image/png', 'image/jpeg', 'image/gif', 'text/markdown', 'text/latex', 'application/json', 'text/plain']

export const isVegaLite = (mime: string) => VEGALITE.test(mime)
const rank = (mime: string) => ORDER.indexOf(isVegaLite(mime) ? 'vegalite' : mime)

export const pick = (data: Record<string, unknown>) =>
  Object.keys(data).filter(m => rank(m) >= 0).sort((a, b) => rank(a) - rank(b))[0]

export const str = (v: unknown) => Array.isArray(v) ? v.join('') : typeof v === 'string' ? v : JSON.stringify(v, null, 2)

export const needsFrame = (mime: string, value: string) =>
  mime === PLOTLY || isVegaLite(mime) || mime === 'application/javascript' || (mime === 'text/html' && /<(script|iframe)\b/i.test(value))

const json = (v: unknown) => JSON.stringify(typeof v === 'string' ? JSON.parse(v) : v).replace(/</g, '\\u003c')
const HEAD = '<!doctype html><meta charset="utf-8"><style>html,body{margin:0;background:transparent;color:#d4d4d4;color-scheme:dark;font:13px -apple-system,BlinkMacSystemFont,sans-serif}</style>'
const RESIZE = '<script>new ResizeObserver(()=>parent.postMessage({frameHeight:document.documentElement.scrollHeight},"*")).observe(document.documentElement)</script>'
const scripts = (...src: string[]) => src.map(s => `<script src="${s}"></script>`).join('')

const bokeh = (js: string) => {
  const version = /root\.Bokeh\.embed/.test(js) && js.match(/"version":"([\d.]+)"/)?.[1]
  if (!version) return ''
  const ids = [...js.matchAll(/"roots":\{([^}]*)\}/g)].flatMap(m => [...m[1].matchAll(/:"([^"]+)"/g)].map(r => r[1]))
  return scripts(...['', '-gl', '-widgets', '-tables'].map(s => `https://cdn.bokeh.org/bokeh/release/bokeh${s}-${version}.min.js`)) + ids.map(id => `<div id="${id}"></div>`).join('')
}

export function frameDoc(mime: string, value: unknown): string {
  const v = mime.match(VEGALITE)
  const body = mime === PLOTLY
    ? `<div id="p"></div>${scripts('https://cdn.plot.ly/plotly-3.1.0.min.js')}<script>const f=${json(value)};Plotly.newPlot("p",f.data,f.layout,{responsive:true})</script>`
    : v
    ? `<div id="v"></div>${scripts(...(Number(v[1]) >= 6 ? ['vega@6', 'vega-lite@6', 'vega-embed@7'] : ['vega@5', `vega-lite@${v[1]}`, 'vega-embed@6']).map(p => `https://cdn.jsdelivr.net/npm/${p}`))}<script>vegaEmbed("#v",${json(value)},{theme:"dark",actions:false})</script>`
    : mime === 'application/javascript'
    ? `${bokeh(str(value))}<div id="out"></div><script>const element=document.getElementById("out");${str(value).replace(/<\/script/gi, '<\\/script')}</script>`
    : str(value)
  return HEAD + body + RESIZE
}

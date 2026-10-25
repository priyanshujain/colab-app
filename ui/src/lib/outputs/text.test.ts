import { describe, expect, it } from 'vitest'
import { ansiToHtml, collapseCarriageReturns, truncate, MAX_LINES } from './text'
import { frameDoc, pick } from './mime'

describe('outputs', () => {
  it('keeps only the last tqdm redraw of each line', () => {
    const tqdm = '\r  0%|          | 0/50 [00:00<?, ?it/s]\r 50%|#####     | 25/50\r100%|##########| 50/50 [00:00<00:00]\r\ndone\n'
    expect(collapseCarriageReturns(tqdm)).toBe('100%|##########| 50/50 [00:00<00:00]\ndone\n')
  })

  it('truncates long output and colors ipython tracebacks without leaking escapes', () => {
    const lines = Array.from({ length: 3000 }, (_, i) => String(i)).join('\n')
    expect(truncate(lines)!.split('\n')).toHaveLength(MAX_LINES)
    expect(truncate('short\noutput')).toBeNull()
    const html = ansiToHtml('\u001b[0;31mZeroDivisionError\u001b[0m: division by <zero>')
    expect(html).toContain('<span class="ansi-red-fg">ZeroDivisionError</span>')
    expect(html).toContain('&lt;zero&gt;')
    expect(html).not.toContain('\u001b')
  })

  it('picks the richest mimetype from real kernel bundles', () => {
    expect(pick({ 'text/plain': 'DataFrame', 'text/html': '<table>' })).toBe('text/html')
    expect(pick({ 'text/plain': 'alt.Chart(...)', 'application/vnd.vegalite.v6.json': {} })).toBe('application/vnd.vegalite.v6.json')
    expect(pick({ 'text/plain': 'IntSlider(value=3)', 'application/vnd.jupyter.widget-view+json': {} })).toBe('application/vnd.jupyter.widget-view+json')
    expect(pick({ 'image/png': 'iVBOR', 'text/plain': '<Figure>' })).toBe('image/png')
  })
})

describe('bokeh', () => {
  it('loads BokehJS and the root div inside the frame of a show() output', () => {
    const js = `(function(root) {
  function embed_document(root) {
  const docs_json = {"eaecf554":{"version":"3.10.1","title":"Bokeh Application","roots":[{"type":"object","name":"Figure","id":"p1008"}]}};
  const render_items = [{"docid":"eaecf554","roots":{"p1008":"c1557f48-16ce"},"root_ids":["p1008"]}];
  void root.Bokeh.embed.embed_items_notebook(docs_json, render_items);
  }
})(window);`
    const doc = frameDoc('application/javascript', js)
    expect(doc).toContain('<script src="https://cdn.bokeh.org/bokeh/release/bokeh-3.10.1.min.js"></script>')
    expect(doc).toContain('<div id="c1557f48-16ce"></div>')
    expect(doc.indexOf('bokeh-3.10.1')).toBeLessThan(doc.indexOf('embed_items_notebook'))
    expect(frameDoc('application/javascript', 'element.textContent = 1')).not.toContain('bokeh')
  })
})

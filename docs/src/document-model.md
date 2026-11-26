# Document Model

The `.ipynb` file on disk is the source of truth. The server holds one copy of each open notebook in memory. It writes the copy back soon after each change, and merges outside changes into it. The code lives in the `nb` crate.

## The notebook type

`nb::Notebook` is a serde model of nbformat 4. A cell is code, markdown or raw. An output is a stream, display data, an execute result or an error.

The model keeps what it does not know. Unknown top level and cell fields go into an `extra` map and come back out on save. Key order is preserved. A file that the app did not change is not written at all.

The model also normalizes two things on load:

- A source or output text that is a list of lines becomes one string. On save it is a list of lines again, as Jupyter writes it.
- A cell without an `id` gets one. The file then moves to `nbformat_minor` 5, where cell ids are part of the format.

The file is written with a one space indent and a final newline, the same as Jupyter. Each cell run also stores its timing in the cell metadata under `execution`.

## Save and reload

`NotebookDoc` wraps an open notebook. It owns the copy in memory and two threads.

<div class="diagram-frame"><svg class="diagram" viewbox="0 0 760 300" role="img" aria-labelledby="doc-title">
<title id="doc-title">The save and reload loop of an open notebook</title>
<defs><marker id="doc-arrow" viewbox="0 0 10 10" refx="9" refy="5" markerwidth="7" markerheight="7" orient="auto-start-reverse"><path class="head" d="M0 0L10 5L0 10z"/></marker></defs>
<rect class="box" x="16" y="40" width="170" height="110" rx="8"/>
<text class="title" x="32" y="66">Changes</text>
<text class="small" x="32" y="88">edits from the UI</text>
<text class="small" x="32" y="106">edits from agents</text>
<text class="small" x="32" y="124">kernel outputs</text>
<text class="small" x="32" y="142">format, rename, fixes</text>
<rect class="hot" x="230" y="60" width="170" height="70" rx="8"/>
<text class="title" x="246" y="88">Notebook in memory</text>
<text class="small" x="246" y="108">active cell, running cells</text>
<rect class="box" x="444" y="60" width="140" height="70" rx="8"/>
<text class="title" x="460" y="88">Writer thread</text>
<text class="small" x="460" y="108">waits 1 s, then saves</text>
<rect class="box" x="628" y="60" width="116" height="70" rx="8"/>
<text class="title" x="644" y="88">train.ipynb</text>
<text class="small" x="644" y="108">on disk</text>
<rect class="box" x="628" y="200" width="116" height="70" rx="8"/>
<text class="title" x="644" y="228">Watcher</text>
<text class="small" x="644" y="248">notify, folder</text>
<rect class="box" x="444" y="200" width="140" height="70" rx="8"/>
<text class="title" x="460" y="228">merge()</text>
<text class="small" x="460" y="248">disk, plus your cell</text>
<rect class="box" x="230" y="200" width="170" height="70" rx="8"/>
<text class="title" x="246" y="228">notebook-changed</text>
<text class="small" x="246" y="248">event to every client</text>
<path class="wire" d="M186 95H226" marker-end="url(#doc-arrow)"/>
<path class="wire" d="M400 95H440" marker-end="url(#doc-arrow)"/>
<text class="small" x="404" y="87">dirty</text>
<path class="wire" d="M584 95H624" marker-end="url(#doc-arrow)"/>
<text class="small" x="560" y="50">tmp file, fsync, rename</text>
<path class="wire" d="M686 130V196" marker-end="url(#doc-arrow)"/>
<text class="small" x="694" y="168">file event</text>
<path class="wire" d="M628 235H588" marker-end="url(#doc-arrow)"/>
<path class="wire" d="M500 200V166H330V134" marker-end="url(#doc-arrow)"/>
<text class="small" x="340" y="160">new copy in memory</text>
<path class="wire" d="M444 235H404" marker-end="url(#doc-arrow)"/>
</svg></div>

*The top row saves changes. The bottom row brings outside changes back in.*

### Save

Every change marks the document dirty and sets a deadline one second away. More changes before the deadline do not move it. So during a long run, the file is written about once a second.

At the deadline the writer serializes the notebook. If the text equals the last text on disk, it writes nothing. Otherwise it writes a hidden temporary file next to the notebook, such as `.train.ipynb.tmp`. It syncs that file to disk and renames it over the notebook. A crash cannot leave half a notebook.

The `save-state` event tells clients when the document becomes dirty or clean. The UI shows a dot next to the file name while it is dirty. Closing a notebook flushes it at once.

### Reload

The watcher thread watches the folder of the notebook, not the file. An editor or git that replaces the file with a rename still fires an event. On each event the thread reads the file. If the text equals the last text that the app wrote, it stops there. So the app ignores its own saves.

Otherwise it parses the file and merges. If the merge result differs from the disk, the writer saves it again a second later.

## Merge rules

`nb::merge` takes the notebook on disk, the notebook in memory, the active cell and the set of running cells. The result starts as the disk version. Then:

1. A running cell keeps its outputs and execution count from memory. Output that streams in is not lost to an outside save.
1. The active cell keeps its whole version from memory. The active cell is the one with a focused editor in some window.
1. If the disk has no such cell, the active cell goes back in. It goes after the cell that came before it in memory, or at the top.

Every other cell comes from disk. There is no reload dialog. If you edit a cell while an agent edits the file, your cell wins and the rest updates.

## Several clients

Many clients can open one notebook: windows on this Mac, a window on another Mac and agents. The server is the only writer of the file. It applies the edits of each connection in arrival order, and sends `notebook-changed` to the other clients.

A client does not get its own `edit_cell` back as an event, so the cursor does not jump. It does get the result of a format, an import sort, a rename or a quick fix. The server made those edits.

## Cell edits

All structural changes go through one type, `CellEdit`:

| Kind | Fields |
|---|---|
| `source` | `cell_id`, `source` |
| `insert` | `cell_id`, `index`, `cell_type` |
| `delete` | `cell_id` |
| `move` | `cell_id`, `index` |
| `change_type` | `cell_id`, `cell_type`. A code cell keeps outputs, a markdown cell drops them. |
| `focus` | `cell_id` or null. Sets the active cell for the merge. It does not change the file. |

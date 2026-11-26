# Notebooks

A notebook is a normal `.ipynb` file on disk. The app reads it, shows its cells and writes your changes back to the same file. Git, your editor and agents see the same file.

## Cells

A notebook has code cells and markdown cells. The toolbar above the cells has these controls:

| Control | What it does |
|---|---|
| Code | Adds a code cell below the selected cell. |
| Markdown | Adds a markdown cell below the selected cell. In a narrow window this moves to the more menu. |
| Run all | Runs every code cell from the top. It stops at the first cell that fails. |
| Interrupt | Interrupts the cell that runs now. |
| Restart | Restarts the kernel. Variables are lost. |
| Format | Formats the cell or the whole notebook with ruff. |
| More | Add Markdown Cell, Restart Kernel, Format Cell, Format Notebook and Organize Imports. |

Each cell has its own buttons: run, move up, move down, delete and a more menu. The more menu converts a code cell to markdown or back. After a run, the cell shows how long the run took and whether it failed.

## Command mode and edit mode

The app copies the Jupyter model. In edit mode the cursor is inside a cell editor. In command mode the cell list has focus, and single keys act on the selected cell.

Press <kbd>Esc</kbd> to leave the editor. Press <kbd>↩</kbd> to edit the selected cell again. The full list is on the [Keyboard shortcuts](keyboard-shortcuts.md) page.

### Markdown cells

Markdown cells show rendered text. To edit one, press <kbd>↩</kbd> or the pencil button. The **Close** button or <kbd>Esc</kbd> renders it again. **Settings > Editor > Double click to edit markdown** also opens the editor on a double click.

## Outputs

For each output the app picks the richest type that it can show. This is the order, from first choice to last:

1. ipywidgets (`application/vnd.jupyter.widget-view+json`)
1. Plotly and Vega-Lite charts
1. `application/javascript`, for example Bokeh
1. HTML, for example pandas DataFrames
1. SVG, PNG, JPEG and GIF images
1. Markdown, LaTeX, JSON and plain text

Charts, scripts and HTML with a `<script>` or `<iframe>` tag run in a sandboxed iframe. One such output cannot freeze the notebook. The iframe loads Plotly, Vega and Bokeh from their public CDNs, so these charts need a network connection.

Text output handles carriage returns, so `tqdm` bars update in place. ANSI colors in tracebacks show as colors. Text longer than 2,000 lines or 1,000,000 characters is cut. A **Show all** button shows the rest.

Code that calls `input()` gets a text field in the cell. A password prompt hides what you type.

## Find

<kbd>⌘</kbd><kbd>F</kbd> opens a find bar that searches every cell. <kbd>↩</kbd> moves to the next match and <kbd>⇧</kbd><kbd>↩</kbd> to the previous one. <kbd>Esc</kbd> closes the bar.

## Rename and file actions

The file name sits in the center of the title bar. Click it to rename the notebook on disk. The app adds `.ipynb` when you leave it out. A name cannot contain `/` or start with a dot.

Right click the file name for **Copy Path** and **Reveal in Finder**. A dot next to the name means the server has not written your latest change yet.

## Clone from Colab

**File > Clone from Colab...** downloads a notebook from a Colab link and saves a copy where you choose. Two link types work:

- `colab.research.google.com/drive/<id>`. The notebook must be shared as **Anyone with the link**.
- `colab.research.google.com/github/<owner>/<repo>/blob/<branch>/<path>`. The app downloads the file from GitHub.

The app names the file after the Colab title of the notebook when it has one. The copy does not sync back to Drive. Clone is available only in windows for this Mac.

## Outside edits

The server watches the file. When git, an editor or a script changes it, the open notebook updates without a dialog. The cell that you edit now keeps your text. The [merge rules](document-model.md#merge-rules) explain the details.

## Windows

One notebook belongs to one window. When you open a notebook that is already open, the app brings its window to the front. **File > New Window** (<kbd>⌘</kbd><kbd>⇧</kbd><kbd>N</kbd>) opens another window, so you can work on two notebooks side by side. **File > Open Recent** keeps the last ten notebooks.

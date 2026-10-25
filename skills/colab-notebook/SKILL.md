---
name: colab-notebook
description: Open, edit and run Jupyter notebooks through the Colab Notebook desktop app on a Colab (or local) runtime, so the user watches cells and outputs live in the app. Use when the user asks to work on, run, debug or extend a notebook, run code on Colab, or mentions "the colab app".
---

# Colab Notebook app

The app hosts the MCP server, so it must be running; the notebook and runtime do not need to be open, the tools below do that. If the `mcp__colab__*` tools fail to connect, launch the app with `open -a "Colab Notebook"`, wait a few seconds and retry. If they are not registered at all, tell the user to run:

```
claude mcp add --transport http --scope user colab http://127.0.0.1:7761/mcp
```

## Working

1. `open_notebook` with the absolute `.ipynb` path (it creates the file if missing and shows it in the app), or `list_notebooks` to see what is already open. Never create or edit notebooks some other way.
2. `kernel_status`, then `connect_kernel` if needed: `colab_cpu` unless the user asks for `colab_gpu`/`colab_tpu` (optional `accelerator` like `T4`) or `local`. It is a no-op when already connected, and a notebook reopened with a saved Colab runtime reattaches by itself, so check status before connecting. A new Colab runtime takes up to a minute.
3. `read_notebook` before changing anything. Cell ids come from here; indexes shift after inserts and deletes.
4. Change code with `insert_cell` (returns the new id) and `edit_cell`. Never edit the `.ipynb` file directly while it is open: the app owns it and saves it.
5. `run_cell` executes on the notebook's kernel, the Colab runtime when connected, and returns the outputs when the cell finishes. Variables live in that kernel, so run setup cells first after a restart.
6. For long jobs, keep cells small and print progress so the user sees it stream. Call `interrupt` if a cell hangs or the user asks to stop.
7. When the work is finished, ask the user whether to shut down the kernel. Call `release_kernel` only if they say yes (or asked to stop the runtime earlier): it stops the Colab runtime using quota but loses all variables.

## Rules

- Work like a person in Colab: build the notebook incrementally, one small step per cell (imports, load data, inspect it, transform, train, evaluate, plot), and run each cell before writing the next so its output guides the next step. Never dump a whole script into one cell.
- Keep cells short, roughly under 15 lines. Split anything longer into separate cells unless it is a single function or class.
- End a cell with the value worth seeing (a `df.head()`, a shape, a metric) so the user follows along.
- Fix a failing cell by editing and re-running it, not by appending a corrected copy below.
- Outputs that are not text show as a mime tag like `[image/png]`; the user sees the real render in the app. Print numbers you need to reason about.
- GPU and TPU runtimes spend the user's compute units: only connect one when asked.
- Do not install packages, mount Drive or download big data without saying so first: it runs on the user's Colab runtime and quota.

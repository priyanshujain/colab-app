# Language Tooling

The editor uses two Astral tools: ty for types, and ruff for lint and format. Both understand notebooks natively. The server runs them as language servers and routes each request to the right one.

## Features

| Feature | Server | How to use it |
|---|---|---|
| Completion | ty | Type in a code cell. |
| Hover | ty | Rest the mouse on a name. |
| Go to definition | ty | <kbd>F12</kbd>, or <kbd>⌘</kbd> click. A target in another file shows in a viewer panel. <kbd>Esc</kbd> closes it. |
| Rename symbol | ty | <kbd>⌘</kbd><kbd>R</kbd>. Renames across all cells. |
| Type errors | ty | Shown as you type. |
| Lint warnings | ruff | Shown as you type, with your project's ruff config. |
| Quick fix | ty and ruff | <kbd>⌘</kbd><kbd>.</kbd> on a diagnostic. |
| Format | ruff | <kbd>⌥</kbd><kbd>⇧</kbd><kbd>F</kbd> for a cell, <kbd>⌘</kbd><kbd>⌥</kbd><kbd>⇧</kbd><kbd>F</kbd> for the notebook. |
| Organize imports | ruff | <kbd>⌥</kbd><kbd>⇧</kbd><kbd>O</kbd>. |

You can change every shortcut in **Settings > Keyboard**. **Settings > Editor > Diagnostics** turns lint warnings and type errors on or off.

On a Colab kernel the app hides ty's `unresolved-import` errors. Packages live on the runtime, so ty on your Mac cannot see them.

## One project, two servers

The server starts one ty and one ruff per project folder, with `ty server` and `ruff server`. The project folder is the nearest parent with a `pyproject.toml`, or else the folder of the notebook. Notebooks in the same project share the two servers.

There is no virtual document and no offset mapping. Each code cell is its own text document with a cell URI:

```text
vscode-notebook-cell:/Users/you/research/train.ipynb#a1b2c3d4
```

The part after `#` is the cell id. Positions in requests and replies are local to the cell. ty treats all cells of a notebook as one module, so a name from cell 1 completes in cell 3.

## Sync

After each change, the engine sends the code cells to both servers:

- The first time, it sends `notebookDocument/didOpen` with all code cells.
- When only text changed, it sends `notebookDocument/didChange` with the new text of each changed cell.
- When cells were added, removed or moved, it closes the notebook and opens it again.

Both servers push diagnostics per cell URI. The server keeps the latest list from each tool, joins them and sends a `diagnostics` event per cell. The UI filters them by the diagnostics settings.

## Edits from the tools

Format, rename, organize imports and quick fixes return LSP text edits keyed by cell URI. The engine applies them to the cells in the server, not in the UI. So every client sees the result, and the file saves as usual. Format first organizes the imports, then sends one formatting request per code cell.

Quick fixes come from both servers. The engine asks each one for code actions in the range, with the diagnostics of that range. Organize imports asks ruff for the `notebook.source.organizeImports` action.

## Managed binaries

The app does not use tools from your `PATH`. At first start the server downloads pinned releases from GitHub into `<data dir>/tools`:

| Tool | Version | Use |
|---|---|---|
| uv | 0.13.1 | Creates `.venv` folders and installs `ipykernel`. |
| ty | 0.0.89 | Types, completion, hover, definition, rename. |
| ruff | 0.17.1 | Lint, format, organize imports. |

Each archive is checked against a SHA-256 checksum in `crates/tooling/src/install.rs` before use. A mismatch stops the install. There are builds for macOS and Linux, on arm64 and x86_64. A new version of a tool is a new pinned version and checksum in that file.

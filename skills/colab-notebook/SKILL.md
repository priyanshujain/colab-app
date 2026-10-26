---
name: colab-notebook
description: Open, edit and run Jupyter notebooks on Colab (or local) runtimes through the Colab Notebook app, so the user watches live. Use for notebook work, running code on Colab, or "the colab app".
---

Uses `mcp__colab__*` tools. The app hosts them: if a call says the app is not running, `open -a "Colab Notebook"` and retry. If they are not registered, tell the user to run `claude mcp add --scope user colab -- "/Applications/Colab Notebook.app/Contents/MacOS/colab-app" mcp`.

- `open_notebook` (absolute path, created if missing), `kernel_status`, then `connect_kernel` only if not connected. Default `colab_cpu`; GPU/TPU only when asked (costs units).
- `read_notebook` for cell ids. Edit only via tools, never the `.ipynb` file.
- Work like Colab: one small step per cell (<15 lines), run it, let output guide the next. No script dumps. End cells with the value worth seeing.
- No markdown cells unless the user asks.
- Fix failures by editing and re-running the cell, not by adding copies.
- Ask before installing packages, mounting Drive or big downloads.
- Non-text outputs show as `[image/png]`; print numbers you need.
- When done, ask the user whether to shut down the kernel; `release_kernel` only on yes.

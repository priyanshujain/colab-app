---
name: colab-notebook
description: Open, edit and run Jupyter notebooks on Colab (or local) runtimes through the Colab Notebook app, so the user watches live. Use for notebook work, running code on Colab, or "the colab app".
---

Uses `mcp__colab__*` tools. The Colab Notebook server hosts them, and `colab-app mcp` starts it when it is not running. If a call says the server did not start, tell the user. If they are not registered, tell the user to run `claude mcp add --scope user colab -- "/Applications/Colab Notebook.app/Contents/MacOS/colab-app" mcp`.

- `open_notebook` (absolute path, created if missing), `kernel_status`, then `connect_kernel` only if not connected. Default `colab_cpu`; GPU/TPU only when asked (costs units).
- Hosts are other machines with their own kernel type, like a Colab runtime type: `MPS` (Apple GPU), `CUDA` (NVIDIA GPU) or `CPU`. `list_hosts` shows each host's type and whether it is available. Pick a host by the kernel type the work needs, for example `MPS` for Apple GPU work, and call `connect_kernel` with `host` (its label). If the host is unavailable, connect Colab with a suitable accelerator instead and tell the user.
- `read_notebook` for cell ids. Edit only via tools, never the `.ipynb` file.
- Work like Colab: one small step per cell (<15 lines), run it, let output guide the next. No script dumps. End cells with the value worth seeing.
- No markdown cells unless the user asks.
- Fix failures by editing and re-running the cell, not by adding copies.
- Ask before installing packages, mounting Drive or big downloads.
- Non-text outputs show as `[image/png]`; print numbers you need.
- When done with a Colab runtime connected, ask whether to release it, and in the same turn start an idle timer: a background Bash `sleep 1800` (`run_in_background: true`). When the timer fires and the user has not replied since, call `release_kernel`, then `close_notebook`, and say so. A user reply cancels that timer. Never leave a Colab runtime idle beyond 30 minutes.
- After `release_kernel`, call `close_notebook` to close its window. With a local kernel, call `close_notebook` when the work is done.

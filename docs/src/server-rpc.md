# Server RPC

Clients talk to `colab-server` over one websocket at `/rpc`. The app connects through the Unix socket on this machine, or over Tailscale for another host. Requests, replies and events share the socket as JSON text frames.

## From a click to the server

<div class="diagram-frame"><svg class="diagram" viewbox="0 0 760 300" role="img" aria-labelledby="rpc-title">
<title id="rpc-title">A request from the UI through the Tauri shell to the server, and the events that come back</title>
<defs><marker id="rpc-arrow" viewbox="0 0 10 10" refx="9" refy="5" markerwidth="7" markerheight="7" orient="auto-start-reverse"><path class="head" d="M0 0L10 5L0 10z"/></marker></defs>
<rect class="box" x="20" y="16" width="160" height="40" rx="8"/>
<text class="title" x="36" y="41">Svelte UI</text>
<rect class="box" x="220" y="16" width="160" height="40" rx="8"/>
<text class="title" x="236" y="41">Tauri call</text>
<rect class="box" x="420" y="16" width="150" height="40" rx="8"/>
<text class="title" x="436" y="41">Server link</text>
<rect class="hot" x="610" y="16" width="134" height="40" rx="8"/>
<text class="title" x="626" y="41">colab-server</text>
<path class="wire dash" d="M100 56V286"/>
<path class="wire dash" d="M300 56V286"/>
<path class="wire dash" d="M495 56V286"/>
<path class="wire dash" d="M677 56V286"/>
<path class="wire" d="M100 86H296" marker-end="url(#rpc-arrow)"/>
<text class="small" x="108" y="78">invoke('call', method, params)</text>
<path class="wire" d="M300 112H491" marker-end="url(#rpc-arrow)"/>
<text class="small" x="308" y="104">host of the window, snake_case</text>
<path class="wire" d="M495 138H673" marker-end="url(#rpc-arrow)"/>
<text class="small" x="503" y="130">{"id", "method", "params"}</text>
<path class="wire" d="M677 170H499" marker-end="url(#rpc-arrow)"/>
<text class="small" x="503" y="162">{"id", "result"} or "error"</text>
<path class="wire" d="M495 196H104" marker-end="url(#rpc-arrow)"/>
<text class="small" x="112" y="188">the promise resolves</text>
<path class="wire" d="M677 236H499" marker-end="url(#rpc-arrow)"/>
<text class="small" x="503" y="228">{"event", "payload"}</text>
<path class="wire" d="M495 262H104" marker-end="url(#rpc-arrow)"/>
<text class="small" x="112" y="254">emitted only to windows of that host</text>
</svg></div>

*The Tauri command `call` is generic. It adds no logic for most methods. It only routes by host.*

## Frames

| Frame | Shape |
|---|---|
| Request | `{"id": 7, "method": "run_cell", "params": {"path": "...", "cell_id": "..."}}` |
| Reply | `{"id": 7, "result": null}` or `{"id": 7, "error": "notebook is not open: ..."}` |
| Event | `{"event": "cell-outputs", "payload": {...}}` |

The client picks the ids. A frame without an `id` is an event. Errors are plain strings with the full cause chain.

## Ordering

Most requests run at once, each in its own task. Nine methods change the notebook, so they run one after another per connection, in arrival order:

`edit_cell`, `rename_notebook`, `format_cell`, `format_notebook`, `organize_imports`, `apply_code_action`, `rename_symbol`, `run_cell`, `run_all`.

A run waits for the edits before it, so it runs the text that you see. The queue does not wait for the run itself to finish. The engine has its own run lock per notebook, so cells of one notebook still run one at a time.

## Events

| Event | Payload and meaning |
|---|---|
| `notebook-changed` | The full notebook after an edit or an outside change. Not sent back to the client whose `edit_cell` caused it. |
| `cell-outputs` | Outputs, execution count, timing and a `running` flag of one cell. |
| `kernel-status` | The status and the kernel choice of a notebook. |
| `save-state` | Whether the notebook has unsaved changes. |
| `diagnostics` | ty and ruff diagnostics of one cell. |
| `comm` | A widget comm message from the kernel. |
| `input-request` | The kernel waits for `input()`. |
| `auth-state` | Signed out, signed in with an email, or client rejected. |
| `tailscale-status` | State of the Tailscale device. Only local clients get it. |
| `agent-open`, `agent-close` | An agent opened or closed a notebook. The Mac app opens or closes a window. |
| `agent-focus` | An agent edited or ran a cell. The window scrolls to it. |

Events go through a broadcast channel with room for 4,096 events. A client that falls further behind is disconnected. It then reconnects and fetches the state again.

## Methods

| Group | Methods |
|---|---|
| Notebooks | `open_notebook`, `open_notebooks`, `rename_notebook`, `edit_cell`, `list_dir`, `read_file`, `clone_link` |
| Runs | `run_cell`, `run_all`, `interrupt`, `restart`, `send_input`, `send_comm` |
| Kernels | `select_kernel`, `kernel_state`, `detach`, `release`, `runtime_started`, `machine`, `host_status` |
| Colab | `begin_sign_in`, `complete_sign_in`, `sign_out`, `auth_state`, `colab_info`, `list_runtimes` |
| Language | `completion`, `hover`, `definition`, `prepare_rename`, `rename_symbol`, `format_cell`, `format_notebook`, `organize_imports`, `code_actions`, `apply_code_action`, `diagnostics` |
| Secrets | `list_secrets`, `set_secret`, `delete_secret` |
| Server | `version`, `can_restart`, `shutdown` |
| Tailscale | `tailscale_status`, `tailscale_enable`, `tailscale_disable`, `tailscale_logout`, `tailscale_proxy` |

`shutdown`, `can_restart` and the five `tailscale_*` methods work only on the local socket. Over Tailscale they fail with "not available over Tailscale".

The sign in has two steps so that it works on a headless host. `begin_sign_in` returns the Google URL with the redirect URI of the client. The client opens it, catches the code on its own loopback port, and sends it with `complete_sign_in`.

## The client side in the app

`src-tauri/src/server.rs` keeps one link per host. Each link is a task that connects, serves and reconnects with a backoff from 100 ms up to 5 seconds. A call waits up to 30 seconds for a local link and 10 seconds for a remote one. Then it fails with "cannot reach".

After each connect, the link fetches the state again. It reopens notebooks that the server no longer has open, and it fetches the kernel state of each window. For the local server it also compares versions, see [Server updates](mac-app-shell.md#server-updates).

Parameters from the UI are in camelCase. The `call` command turns them into snake_case before it sends them.

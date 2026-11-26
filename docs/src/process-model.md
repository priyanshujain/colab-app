# Process Model

Colab Notebook is a client and a server. The server owns every notebook, kernel and language server. The Mac app and agents are clients of it. This split lets kernels outlive windows and lets one notebook serve several clients.

## The processes

<div class="diagram-frame"><svg class="diagram" viewbox="0 0 760 440" role="img" aria-labelledby="proc-title">
<title id="proc-title">Processes of Colab Notebook and how they connect</title>
<defs><marker id="proc-arrow" viewbox="0 0 10 10" refx="9" refy="5" markerwidth="7" markerheight="7" orient="auto-start-reverse"><path class="head" d="M0 0L10 5L0 10z"/></marker></defs>
<rect class="zone" x="196" y="12" width="548" height="116" rx="12"/>
<text class="small" x="212" y="34">Colab Notebook.app (one process)</text>
<rect class="box" x="216" y="46" width="230" height="66" rx="8"/>
<text class="title" x="232" y="72">Svelte UI</text>
<text class="small" x="232" y="92">WKWebView, CodeMirror 6, outputs</text>
<rect class="box" x="496" y="46" width="230" height="66" rx="8"/>
<text class="title" x="512" y="72">Tauri shell (Rust)</text>
<text class="small" x="512" y="92">windows, menus, tray, host links</text>
<path class="wire" d="M446 70H492" marker-start="url(#proc-arrow)" marker-end="url(#proc-arrow)"/>
<text class="small" x="450" y="62">invoke</text>
<text class="small" x="450" y="88">events</text>
<rect class="box" x="16" y="170" width="150" height="50" rx="8"/>
<text class="title" x="32" y="200">Claude Code</text>
<rect class="box" x="16" y="250" width="150" height="50" rx="8"/>
<text class="title" x="32" y="272">colab-app mcp</text>
<text class="small" x="32" y="289">stdio bridge</text>
<path class="wire" d="M91 220V246" marker-end="url(#proc-arrow)"/>
<text class="small" x="98" y="238">stdio</text>
<rect class="hot" x="216" y="170" width="510" height="130" rx="10"/>
<text class="title" x="232" y="196">colab-server</text>
<text class="small" x="232" y="218">Core engine: open notebooks, kernels, Colab auth, secrets, hosts</text>
<text class="small" x="232" y="240">/rpc on a Unix socket (mode 0600)</text>
<text class="small" x="232" y="258">/mcp on 127.0.0.1:7761</text>
<text class="small" x="232" y="276">/rpc and /api/kernels on Tailscale port 80, when on</text>
<path class="wire" d="M166 275H212" marker-end="url(#proc-arrow)"/>
<text class="small" x="170" y="267">HTTP</text>
<path class="wire" d="M611 112V166" marker-start="url(#proc-arrow)" marker-end="url(#proc-arrow)"/>
<text class="small" x="618" y="144">WebSocket, JSON</text>
<rect class="box" x="196" y="352" width="100" height="56" rx="8"/>
<text class="title" x="206" y="376">ipykernel</text>
<text class="small" x="206" y="396">ZeroMQ</text>
<rect class="box" x="306" y="352" width="100" height="56" rx="8"/>
<text class="title" x="316" y="376">Colab</text>
<text class="small" x="316" y="396">WS via proxy</text>
<rect class="box" x="416" y="352" width="100" height="56" rx="8"/>
<text class="title" x="426" y="376">other host</text>
<text class="small" x="426" y="396">over Tailscale</text>
<rect class="box" x="526" y="352" width="100" height="56" rx="8"/>
<text class="title" x="536" y="376">ty, ruff</text>
<text class="small" x="536" y="396">LSP on stdio</text>
<rect class="box" x="636" y="352" width="108" height="56" rx="8"/>
<text class="title" x="646" y="376">.ipynb files</text>
<text class="small" x="646" y="396">watch, write</text>
<path class="wire" d="M246 300V348" marker-end="url(#proc-arrow)"/>
<path class="wire" d="M356 300V348" marker-end="url(#proc-arrow)"/>
<path class="wire" d="M466 300V348" marker-end="url(#proc-arrow)"/>
<path class="wire" d="M576 300V348" marker-end="url(#proc-arrow)"/>
<path class="wire" d="M690 300V348" marker-end="url(#proc-arrow)"/>
<text class="small" x="16" y="380">Each machine runs</text>
<text class="small" x="16" y="396">one colab-server.</text>
</svg></div>

*The app and the agent are both clients. Only the server touches kernels, language servers and notebook files.*

### The app

The Mac app is a Tauri 2 app. The UI is Svelte 5 in the system WKWebView, with no bundled Chromium. The Rust side of the app manages windows, the menu bar, the tray icon and one connection per host. It does not run kernels. See [Mac app shell](mac-app-shell.md).

### The server

`colab-server` is the headless half. On a Mac it is the same binary as the app, started as `colab-app server run`. On Linux it is its own binary. When the app finds no server on the socket, it starts one. The new server writes its output to `server.log`. The server then lives on after the app quits.

One `Core` object in `crates/server/src/engine.rs` holds all state. The RPC listener, the MCP listener and the Tailscale listener all call into the same `Core`. That is why a cell that an agent runs streams into your window.

### The bridge

MCP clients start their servers as child processes on stdio. `colab-app mcp` is a thin bridge from stdio to the HTTP endpoint. It also starts the server when needed. See [The bridge](agents-mcp.md#the-bridge).

## Crates

The Rust workspace has one app crate and six library or binary crates. All share the workspace version.

<div class="diagram-frame"><svg class="diagram" viewbox="0 0 760 330" role="img" aria-labelledby="crate-title">
<title id="crate-title">Dependencies between the crates of the workspace</title>
<defs><marker id="crate-arrow" viewbox="0 0 10 10" refx="9" refy="5" markerwidth="7" markerheight="7" orient="auto-start-reverse"><path class="head" d="M0 0L10 5L0 10z"/></marker></defs>
<rect class="box" x="290" y="16" width="180" height="48" rx="8"/>
<text class="title" x="306" y="38">colab-app</text>
<text class="small" x="306" y="55">src-tauri</text>
<rect class="hot" x="290" y="104" width="180" height="48" rx="8"/>
<text class="title" x="306" y="126">colab-server</text>
<text class="small" x="306" y="143">crates/server</text>
<rect class="box" x="30" y="196" width="150" height="48" rx="8"/>
<text class="title" x="46" y="225">nb</text>
<rect class="box" x="210" y="196" width="150" height="48" rx="8"/>
<text class="title" x="226" y="225">tooling</text>
<rect class="box" x="390" y="196" width="150" height="48" rx="8"/>
<text class="title" x="406" y="225">colab</text>
<rect class="box" x="570" y="196" width="150" height="48" rx="8"/>
<text class="title" x="586" y="225">kernel-local</text>
<rect class="box" x="480" y="270" width="150" height="48" rx="8"/>
<text class="title" x="496" y="299">kernel</text>
<path class="wire" d="M380 64V100" marker-end="url(#crate-arrow)"/>
<path class="wire" d="M330 152L120 192" marker-end="url(#crate-arrow)"/>
<path class="wire" d="M355 152L290 192" marker-end="url(#crate-arrow)"/>
<path class="wire" d="M405 152L460 192" marker-end="url(#crate-arrow)"/>
<path class="wire" d="M440 152L630 192" marker-end="url(#crate-arrow)"/>
<path class="wire" d="M500 244L540 266" marker-end="url(#crate-arrow)"/>
<path class="wire" d="M640 244L600 266" marker-end="url(#crate-arrow)"/>
<path class="wire dash" d="M440 244V258H105V248" marker-end="url(#crate-arrow)"/>
<path class="wire" d="M470 128H740V294H634" marker-end="url(#crate-arrow)"/>
</svg></div>

*An arrow points from a crate to a crate that it uses. The dashed arrow is `colab` to `nb`. `colab-app` also uses `colab` for the sign in, the check of clone links and links to other hosts.*

| Crate | Role | Main files |
|---|---|---|
| `colab-app` | The Tauri app: commands for the UI, windows, menu, tray, one server connection per host. | `src-tauri/src/lib.rs`, `windows.rs`, `server.rs`, `menu.rs`, `tray.rs` |
| `colab-server` | The engine, the RPC and MCP listeners, Tailscale, kernel hosting, hosts, secrets and the CLI. | `engine.rs`, `protocol.rs`, `mcp.rs`, `bridge.rs`, `kernels.rs`, `tailscale.rs` |
| `nb` | The nbformat 4 model, the open document with autosave, the file watcher and the merge. | `crates/nb/src/lib.rs`, `doc.rs` |
| `kernel` | Jupyter message types and the `Kernel` trait that every kernel kind implements. | `crates/kernel/src/lib.rs` |
| `kernel-local` | A local `ipykernel` over ZeroMQ, with HMAC signed messages and uv environments. | `crates/kernel-local/src/lib.rs` |
| `colab` | Google sign in, the Colab backend client, Jupyter over websockets for Colab and remote hosts, the clone of Colab links. | `auth.rs`, `client.rs`, `kernel.rs`, `clone.rs`, `store.rs` |
| `tooling` | Downloads uv, ty and ruff, and runs ty and ruff as notebook language servers. | `install.rs`, `lsp.rs`, `project.rs` |

The UI types come from Rust. Structs marked with `ts-rs` export TypeScript files to `ui/src/bindings/`, so both sides share one definition of each message.

## A cell run, end to end

This is the path of one <kbd>⇧</kbd><kbd>↩</kbd> in a notebook window on this Mac:

1. The UI calls `runCell`. The Tauri command `call` forwards it to the server of the window's host as `{"id", "method": "run_cell", "params"}`.
1. The server queues `run_cell` in order behind earlier edits from the same client, then starts it. See [Ordering](server-rpc.md#ordering).
1. The engine takes the run lock of the notebook and starts a kernel if none is connected.
1. The engine sends an `execute_request` and collects the replies. It coalesces output and sends a `cell-outputs` event at most every 33 ms.
1. Each output change marks the document dirty. The document writes the file one second later.
1. Every client of the server gets the events. The app forwards each event only to the windows of that host.

## Threads and tasks

The server runs on tokio. Each open notebook document also has two plain threads. One watches the file, and one writes it. Language servers and kernels are child processes. The app talks to each server from a tokio task that reconnects with backoff.

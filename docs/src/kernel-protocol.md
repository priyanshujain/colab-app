# Kernel Protocol

Every kernel speaks the standard Jupyter messaging protocol. The server reaches a kernel in one of two ways. It uses ZeroMQ for a kernel on this machine, and a websocket for a kernel behind a Jupyter server. Colab runtimes and remote hosts both use the websocket path.

## The Kernel trait

The `kernel` crate defines the Jupyter `Message` type and one trait. The engine uses only this trait, so it does not care where a kernel runs.

```rust
trait Kernel {
    async fn execute(&self, code: &str) -> Result<Execution>;
    async fn send(&self, msg: Message) -> Result<()>;
    async fn interrupt(&self) -> Result<()>;
    async fn restart(&self) -> Result<()>;
    async fn shutdown(&self) -> Result<()>;
    fn status(&self) -> KernelStatus;
    fn subscribe(&self) -> BoxStream<'static, Message>;
}
```

`execute` returns a stream of the messages whose parent is that request. `subscribe` returns every other message, such as widget updates and Colab requests.

| Implementation | Crate | Transport | Interrupt |
|---|---|---|---|
| `LocalKernel` | `kernel-local` | ZeroMQ: shell, iopub, stdin, control and heartbeat sockets, HMAC SHA-256 signed | `SIGINT` to the process |
| `JupyterKernel<Runtime>` | `colab` | Websocket through Colab's runtime proxy | `POST /api/kernels/<id>/interrupt` |
| `JupyterKernel<RemoteHost>` | `colab` | Websocket to another `colab-server` over Tailscale | Same as Colab |

## Local kernels

`LocalKernel` writes a connection file with five ports and a random key. It starts `python -m ipykernel_launcher -f <file>` from the project `.venv`, with `JPY_PARENT_PID` set to the server. The kernel exits when the server exits.

The environment of the process carries the `PYTHONPATH` entry for the `google.colab.userdata` shim. Secrets arrive later as a silent statement that sets `os.environ`.

## Colab runtimes

The Colab backend lives at `https://colab.research.google.com/tun/m`. Its JSON replies start with the XSSI prefix `)]}'`, which the client strips.

1. `GET assign` returns an XSRF token. `POST assign` sends it back as `X-Goog-Colab-Token`, with the variant, the accelerator and the high RAM flag.
1. The reply has the runtime URL and a runtime proxy token. The runtime is a normal Jupyter server behind Colab's proxy.
1. The client calls `POST /api/kernels`. A new runtime answers with a server error while it boots. The client retries up to 10 times, 3 seconds apart.
1. The client opens `/api/kernels/<id>/channels?session_id=<session>` with the header `X-Colab-Runtime-Proxy-Token`.
1. The server saves the runtime endpoint and kernel id per notebook in `colab-runtimes.json`. That is how a notebook finds its runtime again after a reopen.

The proxy token expires. The client fetches a new one when less than five minutes remain.

### One session per connection

Every message that the client sends carries the same `header.session`, equal to the websocket `session_id`. With a new session per message, Colab's proxy closes the websocket on the second message. A stable session also lets the Jupyter server replay buffered messages after a reconnect.

## Reconnect and backfill

A websocket can drop in the middle of a run. The client handles this in three layers.

<div class="diagram-frame"><svg class="diagram" viewbox="0 0 760 330" role="img" aria-labelledby="re-title">
<title id="re-title">Reconnect of a kernel websocket and backfill from the kernel log</title>
<defs><marker id="re-arrow" viewbox="0 0 10 10" refx="9" refy="5" markerwidth="7" markerheight="7" orient="auto-start-reverse"><path class="head" d="M0 0L10 5L0 10z"/></marker></defs>
<rect class="hot" x="40" y="16" width="200" height="40" rx="8"/>
<text class="title" x="56" y="41">colab-server client</text>
<rect class="box" x="300" y="16" width="200" height="40" rx="8"/>
<text class="title" x="316" y="41">Jupyter server</text>
<rect class="box" x="560" y="16" width="170" height="40" rx="8"/>
<text class="title" x="576" y="41">ipykernel</text>
<path class="wire dash" d="M140 56V316"/>
<path class="wire dash" d="M400 56V316"/>
<path class="wire dash" d="M645 56V316"/>
<path class="wire" d="M140 86H396" marker-end="url(#re-arrow)"/>
<text class="small" x="150" y="78">execute_request, then the silent log hook</text>
<path class="wire" d="M645 112H404" marker-end="url(#re-arrow)"/>
<text class="small" x="420" y="104">iopub output, also appended to the log</text>
<text class="small" x="150" y="140">no message for 30 s: socket is dead</text>
<path class="wire" d="M140 172H396" marker-end="url(#re-arrow)"/>
<text class="small" x="150" y="164">new websocket, same session_id</text>
<path class="wire" d="M400 198H144" marker-end="url(#re-arrow)"/>
<text class="small" x="160" y="190">buffered messages of that session</text>
<path class="wire" d="M140 232H396" marker-end="url(#re-arrow)"/>
<text class="small" x="150" y="224">read /tmp/colab-app/&lt;id&gt;.jsonl from the last offset</text>
<path class="wire" d="M400 258H144" marker-end="url(#re-arrow)"/>
<text class="small" x="160" y="250">missed output lines</text>
<text class="small" x="150" y="292">drop messages whose msg_id was seen,</text>
<text class="small" x="150" y="308">route the rest to their cells</text>
</svg></div>

*The same session id gives a replay from the Jupyter server. The kernel log fills any gap. A set of seen message ids removes duplicates.*

- **Health.** The client pings every 10 seconds. With no message for three ping intervals, it treats the socket as dead.
- **Reconnect.** It opens a new websocket with the same session id, up to 60 times, two seconds apart. The kernel shows `reconnecting` meanwhile. A message that failed to send goes out again on the new socket.
- **Backfill.** After connect, the client runs a silent hook in the kernel. The hook appends each output message to `/tmp/colab-app/<kernel id>.jsonl` on the kernel's machine, up to 100 MB. It tags each message with its byte offset. After a reconnect, the client reads the log from the last offset that it saw, through the Jupyter contents API.

The client keeps the ids of the last 65,536 messages. A message that arrives twice, from the replay and from the log, is routed once.

## Remote hosts

A kernel host serves the Jupyter kernels API itself, on its Tailscale port. `KernelHost` in `crates/server/src/kernels.rs` runs a `LocalKernel` per request and bridges it to websockets. It keeps a ring of the latest frames per session, up to 50 MB or 65,536 frames. It replays the ring on reconnect. See [Network drops](remote-hosts.md#network-drops) for the timeouts.

Each host reports its kernel type at `machine`: `MPS` on Apple Silicon, `CUDA` when `nvidia-smi` names a GPU, else `CPU`. The label is the chip, GPU or CPU model.

## Colab requests

Code from `google.colab` asks the frontend for things. The kernel publishes a `colab_request` message on iopub, with `colab_request_type` and a `colab_msg_id` in the metadata. It then waits on its stdin socket.

The server answers on stdin with an `input_reply`. Its value is `{"type": "colab_reply", "colab_msg_id": N, "data": ...}`, or an `error` field instead of `data`. The server supports `GetSecret` and answers every other type with an error. See [How userdata.get works](secrets.md#how-userdataget-works).

## Widgets and input

ipywidgets talk over comm messages. The engine forwards `comm_open`, `comm_msg` and `comm_close` from the kernel to the UI as `comm` events. The UI sends widget changes back with `send_comm`. The UI renders widgets with `@jupyter-widgets/html-manager`.

Every execute request sets `allow_stdin`. An `input_request` from `input()` becomes a text field in the cell, and `send_input` returns the answer.

## Output handling

The engine writes outputs into the cell as they arrive. It sends a `cell-outputs` event at most once per 33 ms per cell. A fast loop cannot flood the UI. `clear_output(wait=True)` clears on the next output. `update_display_data` finds earlier outputs by their `display_id` and updates them in every cell.

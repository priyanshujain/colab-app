# Remote Hosts

Every machine runs `colab-server`. The server owns the open notebooks and their kernels, and keeps them alive with no window open. Machines reach each other over Tailscale, which is built into the server.

With this you can run a notebook on your Mac on the CUDA kernel of a Linux box. You can also open a window on the Linux box and edit its notebooks from the Mac.

## Two ways to use a host

| You want to | Use | Where the notebook lives |
|---|---|---|
| Use the GPU of another machine | Pick the host in the kernel bar | On this Mac. Only the kernel runs on the host. |
| Work on notebooks of another machine | **File > Connect to Host** | On the host. The window edits it over the network. |

<div class="diagram-frame"><svg class="diagram" viewbox="0 0 760 320" role="img" aria-labelledby="remote-title">
<title id="remote-title">A Mac and a Linux host connected over Tailscale</title>
<defs><marker id="remote-arrow" viewbox="0 0 10 10" refx="9" refy="5" markerwidth="7" markerheight="7" orient="auto-start-reverse"><path class="head" d="M0 0L10 5L0 10z"/></marker></defs>
<rect class="zone" x="16" y="16" width="320" height="288" rx="12"/>
<text class="small" x="32" y="40">This Mac (notebook host)</text>
<rect class="zone" x="424" y="16" width="320" height="288" rx="12"/>
<text class="small" x="440" y="40">anton (Linux box, kernel host)</text>
<rect class="box" x="40" y="58" width="272" height="56" rx="8"/>
<text class="title" x="56" y="82">Colab Notebook app</text>
<text class="small" x="56" y="101">windows for local and remote notebooks</text>
<rect class="hot" x="40" y="186" width="272" height="72" rx="8"/>
<text class="title" x="56" y="210">colab-server</text>
<text class="small" x="56" y="229">owns train.ipynb, saves every edit</text>
<text class="small" x="56" y="246">stores the kernel id of the host</text>
<rect class="hot" x="448" y="58" width="272" height="72" rx="8"/>
<text class="title" x="464" y="82">colab-server</text>
<text class="small" x="464" y="101">Tailscale device anton-colab</text>
<text class="small" x="464" y="118">keeps a replay ring per session</text>
<rect class="box" x="448" y="202" width="272" height="56" rx="8"/>
<text class="title" x="464" y="226">ipykernel</text>
<text class="small" x="464" y="245">runs in ~/project on the host's disk</text>
<path class="wire" d="M176 114V182" marker-end="url(#remote-arrow)"/>
<text class="small" x="184" y="152">Unix socket /rpc</text>
<path class="wire" d="M312 214H380V100H444" marker-end="url(#remote-arrow)"/>
<text class="small" x="318" y="232">/api/kernels</text>
<path class="wire dash" d="M312 80H444" marker-end="url(#remote-arrow)"/>
<text class="small" x="318" y="72">Connect to Host: /rpc</text>
<path class="wire" d="M584 130V198" marker-end="url(#remote-arrow)"/>
<text class="small" x="592" y="168">ZeroMQ</text>
<text class="small" x="388" y="162">Tailscale</text>
<text class="small" x="388" y="176">port 80</text>
</svg></div>

*The kernel bar path uses the kernel websocket. Connect to Host uses the full RPC of the host. Both travel inside Tailscale.*

## Turn on Tailscale

Tailscale is off by default. Each server joins your tailnet as its own device through the embedded libtailscale. The machine does not need the Tailscale app.

On the Mac, open **Settings > Server** and turn on **Tailscale**. Set the **Device name** first if you want a name other than the default. The default is the short host name plus `-colab`, for example `anton-colab`. Press **Sign in...** and finish the sign in in your browser.

On a headless machine, use the command line:

```sh
colab-server tailscale on --hostname anton-colab
colab-server tailscale status
```

`on` prints a sign in link when the device needs one. `off` stops the device and keeps its sign in. `logout` signs it out.

## Add a host

Open **Settings > Remote Hosts** and press **Add Remote Host...**. Give it a name and an address. The address is the device name, its full MagicDNS name or its Tailscale IP. The port is 80 unless you write `host:port`. The app does not discover hosts on its own.

```sh
colab-server hosts add anton anton-colab
colab-server hosts list
colab-server hosts remove anton
```

The app and `colab-server hosts` share one list. Each host shows its kernel type, for example `CUDA · NVIDIA L4`, and its state. The info button shows the path (direct or relay) and the latency.

A host is available when it answers. A host that sleeps, is offline or has Tailscale off is unavailable. It still shows the kernel types that it reported last.

## Run on a remote kernel

Pick the host in the place menu of the kernel bar and press **Connect**. The kernel host runs the kernel in `~/<project>` when that folder exists. `<project>` is the name of the project folder of the notebook. Otherwise the kernel runs in `<data dir>/kernels/<project>` with its own `.venv`.

Code on a remote kernel sees the disk of the kernel host, the same as Colab. Files do not sync.

### Network drops

The notebook host stores the kernel id and reconnects after a drop. The kernel host keeps the latest kernel messages of each session in a ring. The ring holds up to 50 MB or 65,536 frames. On reconnect it replays the ring, and the notebook host skips messages that it already has. Output is lost only when the notebook host restarts.

The kernel host drops a client that it has not heard from for 60 seconds. It shuts down a kernel with no client that was not busy for 24 hours. A kernel that a Mac hosts for another machine holds a sleep assertion while busy.

## Open a window on a host

**File > Connect to Host** lists your saved hosts. The new window belongs to that host. Its Open dialog browses the disk of the host, and its title shows the host name in brackets. When the last window of a host closes, the app drops its connection to that host.

Settings always opens for this Mac. Clone from Colab is not available in a host window.

## Security

The server serves `/rpc` and the Jupyter kernels API on port 80 as plain HTTP. WireGuard already encrypts the traffic. These rules protect the port:

- Only the Tailscale user who signed this device in gets in, and only from an untagged device. The server checks each connection with the Tailscale `whois` API.
- The server refuses every request with an `Origin` header. It also refuses a request whose `Host` is not a name or IP of this device. A browser cannot reach the port.
- Over Tailscale, `shutdown`, `can_restart` and the `tailscale_*` methods do not work. The MCP endpoint is never served over Tailscale.

A server dials other hosts through the SOCKS5 proxy of its own Tailscale device. When the device is off, it dials directly.

## Run the server on Linux

Build the server binary. The build needs Rust and Go, see [Building and releasing](building.md).

```sh
cargo build --release -p colab-server
cp target/release/colab-server ~/.local/bin/
```

Save this unit as `~/.config/systemd/user/colab-server.service`:

```ini
[Unit]
Description=Colab Notebook server

[Service]
ExecStart=%h/.local/bin/colab-server run
Restart=on-failure

[Install]
WantedBy=default.target
```

Start it, and let it run with no login session:

```sh
systemctl --user daemon-reload
systemctl --user enable --now colab-server
loginctl enable-linger "$USER"
```

Run `colab-server tailscale on` and open the link that it prints. Read the log with `journalctl --user -u colab-server`. On Linux the data folder is `$XDG_DATA_HOME/colab-app`. Secrets and Colab tokens go in `0600` files in that folder.

An agent on the Linux box registers the MCP server like this:

```sh
claude mcp add --scope user colab -- ~/.local/bin/colab-server mcp
```

Colab sign in also works on a headless host. The server makes the sign in link, the client opens it and sends the code back.

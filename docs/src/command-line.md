# Command Line

The server has a small command line. On Linux it is the `colab-server` binary. On a Mac the app binary contains the same commands.

## Two binaries, one set of commands

| Binary | Where | Commands |
|---|---|---|
| `colab-server` | Built with `cargo build -p colab-server`, for Linux and macOS. | All commands below. |
| `colab-app` | `/Applications/Colab Notebook.app/Contents/MacOS/colab-app` | `colab-app mcp`, and `colab-app server` followed by any command below. |

With no argument, `colab-app` starts the Mac app. For example, this stops the server of the Mac app:

```sh
"/Applications/Colab Notebook.app/Contents/MacOS/colab-app" server stop
```

## run

```sh
colab-server run [--tailscale] [--tailscale-hostname <name>]
```

Starts the server in the foreground and logs to stderr. It fails when a server already listens on the socket. `--tailscale` turns the Tailscale device on at start. `--tailscale-hostname` sets its device name.

The server listens on a Unix socket for clients and on `127.0.0.1:7761` for MCP. With Tailscale on, it also listens on port 80 of its Tailscale device. See [Files and locations](files.md) for the socket path.

## stop

```sh
colab-server stop
```

Asks the running server to shut down and waits up to 10 seconds for it to stop. Local kernels stop with it. A Colab runtime stays assigned, and the server reattaches to it when it opens the notebook again.

## mcp

```sh
colab-server mcp
colab-app mcp
```

Runs the stdio bridge for MCP clients. The bridge starts the server when it does not answer. See [The bridge](agents-mcp.md#the-bridge).

## hosts

```sh
colab-server hosts add <label> <address>
colab-server hosts list
colab-server hosts remove <label>
```

Edits the list of saved hosts. The Mac app uses the same list in **Settings > Remote Hosts**. A label must be unique and cannot be `local`. Each command prints the list after the change, one host per line.

## tailscale

```sh
colab-server tailscale on [--hostname <name>]
colab-server tailscale off
colab-server tailscale logout
colab-server tailscale status
```

Controls the Tailscale device of the running server. `on` waits up to 60 seconds for the device to start or to give a sign in link. It then prints one line:

```text
Tailscale is running as anton-colab.example.ts.net for you@example.com
Sign in to Tailscale as anton-colab: https://login.tailscale.com/...
```

`off` stops the device and keeps its sign in. `logout` signs the device out. These commands need a running server, and they work only on the local socket.

## Exit codes

| Code | Meaning |
|---|---|
| 0 | The command worked. |
| 1 | The command failed. The error goes to stderr with the prefix `colab-server:`. |
| 2 | Unknown command, or an unknown flag for `run`. The usage line goes to stderr. |

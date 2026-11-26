# Troubleshooting

Most problems show up as a message in the window or in a log. This page quotes the messages as the code writes them, so you can search for them.

## Read the logs first

| Log | Where |
|---|---|
| Server on a Mac | `~/Library/Application Support/dev.colab-app/server.log` |
| App on a Mac | `~/Library/Logs/dev.colab-app/` |
| Server on Linux | `journalctl --user -u colab-server` |

To watch the server live, stop it in **Settings > Server** and run it in a terminal:

```sh
"/Applications/Colab Notebook.app/Contents/MacOS/colab-app" server run
```

## The server

### The kernel shows "reconnecting" and nothing runs

The app lost its link to the server. It retries on its own, with a backoff up to five seconds. Check the state in the menu bar icon. If it says Stopped, press **Start Server** in **Settings > Server**. If the server does not stay up, read `server.log`.

### "cannot reach the Colab Notebook server"

A call waited 30 seconds for the local server. For a host window, the message names the host and the wait is 10 seconds. Start the server, or check that the host is awake and on Tailscale.

### "The Colab Notebook server still runs version ..."

You updated the app, but kernels still run on the old server. The server restarts with the new version when no kernel is alive. Release or disconnect your kernels to make it happen now. See [Server updates](mac-app-shell.md#server-updates).

### "colab-server is already running on ..."

`colab-server run` found a live server on the socket. Stop that one first with `colab-server stop`.

## Colab

### "Sign in to Colab first (Settings)"

The browser sign in did not finish. Open **Settings > Account** and press **Sign in with Google**.

### Google rejects the client

The Account section shows the error and asks for your own client. Create a desktop OAuth client in a Google Cloud project. Enter its id and secret in **Settings > Account** and sign in again.

### "this Google account has no eligible accelerator of that type"

An agent asked for a GPU or TPU without a name. Your account can use none of that type. Use the CPU runtime, or name an accelerator that the kernel bar lists.

### Drive mount or authenticate_user fails

These calls ask the Colab web page for credentials. The app answers only secret requests. Every other request type fails with "`<type> is not supported by this app`". Fetch data from a URL, Hugging Face or a bucket, and keep keys in [Secrets](secrets.md).

### A runtime still uses compute units

Detach keeps the runtime. Press **Release** in the kernel bar to give it back. An agent with the skill calls `release_kernel`. See [Detach and release](kernels.md#detach-and-release).

## Kernels and the editor

### The local kernel does not start

The server needs uv from its managed tools. At first start it downloads them from GitHub. Without a network, or after a checksum mismatch, `server.log` shows "installing tools" with the cause. Restart the server when the network is back.

The kernel uses the `.venv` of the project folder. A broken `.venv` breaks the kernel too. Fix it with uv in a terminal.

### ty reports imports that do work

On a Colab kernel the app hides `unresolved-import`, because packages live on the runtime. On this Mac, install the package in the `.venv` of the project, or turn off **Type errors** in **Settings > Editor**.

### A chart is blank

Plotly, Vega-Lite and Bokeh outputs load their libraries from public CDNs. Without a network these outputs stay empty.

## Remote hosts

### A host shows as unavailable

The host sleeps, is offline or has Tailscale off. Check `colab-server tailscale status` on the host. Both machines must be signed in to the same Tailscale user, and the client must be an untagged device.

### "`<method>` is not available over Tailscale"

Some methods only work on the local socket: `shutdown`, `can_restart` and the Tailscale methods. Run them on the host itself.

## Agents

### The agent has no colab tools

Register the server with `claude mcp add`, see [Setup](agents-mcp.md#setup). Start a new agent session after that.

### "The Colab Notebook server is not running and did not start."

The bridge tried to start the server for 30 seconds. Read `server.log`, or run the server in a terminal to see the error.

### No window appears when the agent opens a notebook

That is the default. Turn on **Settings > Editor > Open notebooks that agents use**.

## Clone from Colab

| Message | Fix |
|---|---|
| "not a Colab notebook link" | Use a `colab.research.google.com/drive/...` or `.../github/...` link. |
| "the notebook is not shared publicly" | In Colab, use Share and set General access to Anyone with the link. |
| "the GitHub file was not found" | Check the owner, repo, branch and path in the link. |

## Appearance

### The window is not translucent

Check that **Translucent window** is on in **Settings > Appearance**. The app also turns the effect off when **Reduce transparency** is on in the macOS accessibility settings.

### macOS asks for the Keychain password

The Keychain items belong to the signed release. A build with another signature, such as `just dev`, makes macOS ask. Allow it, or use the release build.

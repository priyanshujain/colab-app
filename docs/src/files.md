# Files

This page lists what the app and the server keep on disk. On a Mac the data folder is `~/Library/Application Support/dev.colab-app`. On Linux it is `$XDG_DATA_HOME/colab-app`, usually `~/.local/share/colab-app`.

The app has no analytics and no crash reporting of its own.

## Data folder

| Path in the data folder | Contents |
|---|---|
| `server.log` | Output of a server that the app or the bridge started. |
| `hosts.json` | Saved remote hosts, as label and address pairs. |
| `remote-kernels.json` | The host kernel of each notebook, for reattach after a reopen. |
| `tailscale.json` | `{"enabled", "hostname"}` of the Tailscale device. |
| `tailscale/` | State of the Tailscale device, with its sign in. |
| `tools/` | Pinned uv, ty and ruff, one folder per tool and version. |
| `python/` | The `google.colab.userdata` shim for local kernels. |
| `kernels/<project>/` | Working folder, with its own `.venv`, of a kernel that this machine hosts for another machine. |
| `signed-out` | A marker. While it exists, the server does not import the Colab CLI token. |
| `app.json` | Mac app only: `{"open_agent_notebooks"}`. |
| `recent.json` | Mac app only: the Open Recent list, up to ten host and path pairs. |
| `secrets.json`, `colab-token.json` | Linux only, in `0600` files. On a Mac these live in the Keychain. |

## Other locations on a Mac

| Location | Contents |
|---|---|
| `~/Library/Application Support/colab-app/run/server.sock` | The server socket, mode `0600` in a `0700` folder. File permissions are the only auth. |
| `~/Library/Application Support/colab-app/colab-runtimes.json` | The Colab runtime and kernel id of each notebook. |
| Keychain, service `colab-app`, account `google-oauth` | Your Google tokens and OAuth client. |
| Keychain, service `colab-app`, account `secrets` | All secrets, as one JSON map. |
| `~/Library/Logs/dev.colab-app/` | The log of the app process. Files rotate at 5 MB and three are kept. |
| App web storage, key `settings` | Editor and appearance settings, and the shortcuts. |

## Other locations on Linux

| Location | Contents |
|---|---|
| `$XDG_RUNTIME_DIR/colab-app/server.sock` | The server socket. Without `XDG_RUNTIME_DIR` it is `run/server.sock` in the data folder. |
| `colab-runtimes.json` in the data folder | The Colab runtime and kernel id of each notebook. |
| `journalctl --user -u colab-server` | The server log under systemd. |

## Next to your notebooks

| Path | Contents |
|---|---|
| `.<name>.ipynb.tmp` | Exists for a moment during each save, then becomes the notebook. |
| `.venv/` in the project folder | Created by uv for the local kernel if it is missing. |
| `pyproject.toml` | Gets `ipykernel` as a dev dependency when the project lacks it. |

## Read, never written

`~/.config/colab-cli/token.json` is the token of Google's Colab CLI. When the Keychain has no Google token, the server imports this file and saves the token in the Keychain. It skips the import after you sign out in the app. It never writes to that file.

## On the kernel's machine

A Colab runtime or a kernel host keeps `/tmp/colab-app/<kernel id>.jsonl`, a log of kernel output for [backfill](kernel-protocol.md#reconnect-and-backfill). It stops at 100 MB and goes away with the VM or the next cleanup of `/tmp`.

# Kernels

Each notebook has one kernel. The kernel can run on this Mac, on a Google Colab runtime, or on one of your other machines. You switch the place for the whole notebook, then run the cells again.

## The kernel bar

The kernel bar sits above the cells. It has a place menu, a kernel type menu and one or two buttons.

| Place | Kernel types | Buttons when connected |
|---|---|---|
| This Mac | The type of this machine, for example `MPS` with the chip name. | **Disconnect** shuts the kernel down. |
| A saved host | The type that the host reports: `MPS`, `CUDA` or `CPU`. | **Detach** and **Release**. |
| Colab | `CPU`, plus each GPU and TPU that your account can use. A **High RAM** box sits next to it. | **Detach** and **Release**. |

In a wide window the bar also shows one state word, or "Not connected". The states are `connecting`, `idle`, `busy`, `reconnecting`, `disconnected` and `dead`. A host that does not answer shows as `unavailable`.

## Kernel details

A status dot sits at the right of the title bar. Hover over it, or move keyboard focus to it, to see the kernel details. This works for every kernel kind, while the kernel is idle or busy. The popover shows:

- The kernel name: the machine or host name with its kernel type id, such as `MPS`, or "Colab" with the accelerator.
- For this Mac and saved hosts, the kernel type with its chip or GPU label. For Colab, "High RAM" when you picked it.
- The state, for example "Kernel idle".
- "Connected for" and the time since the server connected the kernel. For Colab it says "Running for" and counts from the runtime assignment.
- For Colab only: the compute units used so far, the hourly rate and the units left on your account.

When you run a cell with no kernel, the server starts one. It uses this Mac, or the saved host that the notebook last used. The server never starts a Colab runtime on its own.

## This Mac

A local kernel is an `ipykernel` process. The server talks to it over the Jupyter ZeroMQ protocol.

1. The server finds the project folder. That is the nearest parent folder of the notebook with a `pyproject.toml`. Without one, it is the folder of the notebook.
1. If `.venv/bin/python` does not exist there, the server runs `uv venv`.
1. If `ipykernel` is missing, the server runs `uv add --dev ipykernel` in a project with a `pyproject.toml`. In other folders it runs `uv pip install ipykernel`.
1. The kernel starts with the project folder as its working folder.

On Apple Silicon the kernel type is `MPS`, so PyTorch code can use the Apple GPU. uv is the only environment and package manager that the app uses.

While a kernel on the Mac is busy, the server holds a macOS sleep assertion. The assertion is named "Colab Notebook kernel running for" and the notebook file name.

## Colab runtimes

Pick **Colab**, a kernel type and press **Connect**. The server asks Colab for a runtime on your Google account, then creates a kernel on it. A new runtime can take a minute to boot. The server retries kernel creation while the runtime boots.

Code on a Colab kernel runs on the Colab VM. It cannot see files on your Mac. Fetch data from a URL, Hugging Face or a cloud bucket, so the notebook runs the same on every kernel.

> GPU and TPU runtimes use Colab compute units. The CPU runtime is the default for agents. The [skill](agents-mcp.md#the-skill) tells agents to use a GPU only when you ask.

### Usage

While a Colab kernel is connected, the kernel bar shows how long the runtime has run. The [kernel details](#kernel-details) popover adds the compute units used so far, the hourly rate and your balance.

### Lost connections

Colab's proxy sometimes closes the kernel websocket. The server pings every 10 seconds. After three missed pings it opens a new websocket with the same session id. The Jupyter server then replays the buffered output. It tries 60 times, two seconds apart, and shows `reconnecting` meanwhile. The runtime proxy token is refreshed when less than five minutes remain.

## Detach and release

These two buttons look alike but cost different things.

| Action | Colab | Saved host |
|---|---|---|
| Detach | Drops the connection. The runtime and its variables stay, and it still uses compute units until Colab stops it. | Drops the connection. The kernel keeps running on the host. |
| Release | Gives the runtime back to Colab. The VM, its disk and all variables are gone. | Shuts the kernel down on the host. Variables are lost. |

The server records which runtime each notebook uses. When you open the notebook again, it reattaches to that runtime if Colab still has it. When you press Connect again with the same runtime type, the server reuses the detached runtime. With a different type, it releases the old runtime first.

## Remote kernels

A saved host is another machine that runs `colab-server` with Tailscale on. Its kernel runs on that machine's disk, in `~/<project>` when that folder exists. Otherwise it runs in a folder of its own on the host. Files do not sync. The [Remote hosts and Tailscale](remote-hosts.md) page explains the setup.

## Environment of every kernel

Every kernel gets your [secrets](secrets.md) as environment variables, also after a restart. Local kernels also get a small `google.colab.userdata` module, so code written for Colab works unchanged.

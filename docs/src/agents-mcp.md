# Agents

The server has an MCP server. An agent such as Claude Code uses it to open notebooks, connect kernels, and edit and run cells. The agent works on the same live notebook as you, so you watch each cell run.

## What agents can do

| You ask | The agent |
|---|---|
| "Open ~/research/probe.ipynb and load the dataset." | Opens the notebook, connects a Colab CPU runtime, inserts a small cell and runs it. |
| "Train this on a Colab T4." | Connects a Colab GPU runtime with the T4 accelerator, then runs the training cell. |
| "Run it on anton's GPU." | Lists the saved hosts, picks the host by its kernel type and connects to it. |
| "That cell failed, fix it." | Edits the same cell and runs it again, so the notebook does not fill with copies. |
| "Use my Hugging Face token." | Reads it from the kernel environment, where your [secrets](secrets.md) already are. |
| "I'm done." | Asks whether to release the Colab runtime, then releases it and closes the notebook. |

## Setup

Register the server with Claude Code once:

```sh
claude mcp add --scope user colab -- "/Applications/Colab Notebook.app/Contents/MacOS/colab-app" mcp
```

This runs the [bridge](#the-bridge), which starts the server when it does not answer. Then install the `colab-notebook` skill, as the [Skills](skills.md) page shows.

A client that speaks streamable HTTP can also connect straight to the server while it runs:

```sh
claude mcp add --transport http --scope user colab http://127.0.0.1:7761/mcp
```

That form does not start the server for you.

## Watch the agent work

Every tool goes through the same engine as the UI. The window shows what the agent opens, and the kernel bar shows the kernel that it connects. Output streams into the window while `run_cell` waits.

The app selects and scrolls to each cell that the agent edits or runs. It does not do this while you type in a cell. You can edit cells yourself at any time, and the agent sees your changes on its next `read_notebook`.

By default an agent does not open windows. Turn on **Settings > Editor > Open notebooks that agents use** to change that. The app then shows a window when an agent opens a notebook, and closes it when the agent calls `close_notebook`.

## Tools

| Tool | Arguments | What it does |
|---|---|---|
| `open_notebook` | `path` | Opens the notebook. Creates an empty one when the file is missing. Reattaches its saved Colab runtime. The path must be absolute and end in `.ipynb`. |
| `list_notebooks` | none | Lists the open notebooks. |
| `connect_kernel` | `path`, `runtime` or `host`, `accelerator`, `high_ram` | Connects a kernel. `runtime` is `local`, `colab_cpu`, `colab_gpu` or `colab_tpu`. `host` is the name of a saved host. Does nothing when the same kernel is connected. |
| `kernel_status` | `path` | Returns the status, the kernel type and the host. |
| `list_hosts` | none | Lists saved hosts with their kernel type and whether each is available. |
| `release_kernel` | `path` | Releases the Colab runtime, or shuts down the local or host kernel. Variables are lost. |
| `close_notebook` | `path` | Saves and closes the notebook and its window. A Colab runtime or host kernel keeps running unless `release_kernel` ran first. |
| `read_notebook` | `path` | Returns every cell with its id, source and text outputs. |
| `insert_cell` | `path`, `index`, `source`, `markdown` | Inserts a code cell, or a markdown cell, and returns its id. An index past the end appends it. |
| `edit_cell` | `path`, `cell_id`, `source` | Replaces the source of a cell. |
| `delete_cell` | `path`, `cell_id` | Deletes a cell. |
| `run_cell` | `path`, `cell_id` | Runs a code cell, waits, and returns its outputs as text. |
| `interrupt` | `path` | Interrupts the cell that runs now. |
| `set_env` | `path`, `env` | Sets environment variables in the kernel. Starts the local kernel if needed. The values are not stored and last until the kernel restarts. |

For `colab_gpu` and `colab_tpu` without an `accelerator`, the server picks the first one that your account can use. Outputs that are not text, such as images, come back as a marker like `[image/png]`. ANSI codes are removed from tracebacks.

## The skill

The `colab-notebook` skill tells the agent how to work in a notebook the Colab way. These are its main rules:

- Open the notebook, check `kernel_status`, and connect only when no kernel is connected. Use `colab_cpu` by default. Use a GPU or TPU only when you ask, because they cost compute units.
- Pick a saved host by the kernel type that the work needs. When the host is unavailable, use Colab and say so.
- Edit only through the tools, never the `.ipynb` file.
- Write one small step per cell, under 15 lines, and run it. Add no markdown cells unless you ask.
- Fix a failed cell in place. Ask before package installs, Drive mounts and big downloads.
- After the work on a Colab runtime, ask whether to release it. Release it after 30 idle minutes without a reply.

## The bridge

Claude Code starts MCP servers as child processes that speak MCP over stdio. `colab-app mcp` and `colab-server mcp` are small bridges. They forward each stdio message to `http://127.0.0.1:7761/mcp` and print the replies.

<div class="diagram-frame"><svg class="diagram" viewbox="0 0 760 210" role="img" aria-labelledby="bridge-title">
<title id="bridge-title">Claude Code talks to the server through the stdio bridge</title>
<defs><marker id="bridge-arrow" viewbox="0 0 10 10" refx="9" refy="5" markerwidth="7" markerheight="7" orient="auto-start-reverse"><path class="head" d="M0 0L10 5L0 10z"/></marker></defs>
<rect class="box" x="16" y="40" width="150" height="60" rx="8"/>
<text class="title" x="32" y="66">Claude Code</text>
<text class="small" x="32" y="85">with the skill</text>
<rect class="box" x="226" y="40" width="160" height="60" rx="8"/>
<text class="title" x="242" y="66">colab-app mcp</text>
<text class="small" x="242" y="85">stdio bridge</text>
<rect class="hot" x="446" y="28" width="150" height="84" rx="8"/>
<text class="title" x="462" y="54">colab-server</text>
<text class="small" x="462" y="73">MCP on 127.0.0.1:7761</text>
<text class="small" x="462" y="90">same engine as the UI</text>
<rect class="box" x="640" y="20" width="104" height="44" rx="8"/>
<text class="title" x="656" y="47">kernel</text>
<rect class="box" x="640" y="80" width="104" height="44" rx="8"/>
<text class="title" x="656" y="107">.ipynb</text>
<rect class="box" x="446" y="150" width="150" height="44" rx="8"/>
<text class="title" x="462" y="177">app window</text>
<path class="wire" d="M166 70H222" marker-end="url(#bridge-arrow)"/>
<text class="small" x="176" y="62">stdio</text>
<path class="wire" d="M386 70H442" marker-end="url(#bridge-arrow)"/>
<text class="small" x="396" y="62">HTTP</text>
<path class="wire" d="M596 52H636" marker-end="url(#bridge-arrow)"/>
<path class="wire" d="M596 96H636" marker-end="url(#bridge-arrow)"/>
<path class="wire dash" d="M521 112V146" marker-end="url(#bridge-arrow)"/>
<text class="small" x="530" y="134">events</text>
<path class="wire dash" d="M306 100V128"/>
<text class="small" x="248" y="146">starts the server</text>
<text class="small" x="248" y="161">when it is down</text>
</svg></div>

*The agent and the app window share one engine, so both see the same notebook and kernel.*

The bridge does two more things:

- When the server does not answer, the bridge starts it and waits up to 30 seconds. If the server still does not start, the tool call fails with a message that names `server.log`.
- When the server restarts, the old MCP session is gone. The bridge opens a new session and sends the call again, so the agent does not notice.

## Security

The MCP server listens on loopback only. It rejects requests with a browser `Origin`, so web pages cannot reach it. It has no token. Any local process that runs as you can use it. Such a process can read your notebooks anyway. The MCP endpoint is never served over Tailscale.

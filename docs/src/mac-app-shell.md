# Mac App Shell

The Rust half of the Mac app lives in `src-tauri/src`. It owns the windows, the menus, the menu bar icon and the links to servers. It holds no notebook state, so a crash of a window loses nothing.

## Windows and hosts

Every notebook window belongs to one host. The host is part of the window label, so the app needs no extra table to know it.

| Label | Window |
|---|---|
| `main`, `window-2` | A notebook window for this Mac. |
| `remote-616e746f6e-3` | A window for the host `anton`. The middle part is the host name in hex. |
| `settings` | The one Settings window. It always talks to this Mac. |

<div class="diagram-frame"><svg class="diagram" viewbox="0 0 760 250" role="img" aria-labelledby="win-title">
<title id="win-title">Windows map to hosts, and each host has one server link</title>
<defs><marker id="win-arrow" viewbox="0 0 10 10" refx="9" refy="5" markerwidth="7" markerheight="7" orient="auto-start-reverse"><path class="head" d="M0 0L10 5L0 10z"/></marker></defs>
<rect class="box" x="16" y="20" width="200" height="44" rx="8"/>
<text class="title" x="32" y="40">main</text>
<text class="small" x="32" y="56">train.ipynb</text>
<rect class="box" x="16" y="80" width="200" height="44" rx="8"/>
<text class="title" x="32" y="100">window-2</text>
<text class="small" x="32" y="116">eval.ipynb</text>
<rect class="box" x="16" y="140" width="200" height="44" rx="8"/>
<text class="title" x="32" y="160">settings</text>
<text class="small" x="32" y="176">always local</text>
<rect class="box" x="16" y="200" width="200" height="44" rx="8"/>
<text class="title" x="32" y="220">remote-616e746f6e-3</text>
<text class="small" x="32" y="236">sweep.ipynb [anton]</text>
<rect class="hot" x="300" y="70" width="190" height="70" rx="8"/>
<text class="title" x="316" y="98">link: local</text>
<text class="small" x="316" y="118">Unix socket, starts the server</text>
<rect class="hot" x="300" y="186" width="190" height="58" rx="8"/>
<text class="title" x="316" y="210">link: anton</text>
<text class="small" x="316" y="230">closed with its last window</text>
<rect class="box" x="560" y="70" width="184" height="70" rx="8"/>
<text class="title" x="576" y="98">colab-server</text>
<text class="small" x="576" y="118">this Mac</text>
<rect class="box" x="560" y="186" width="184" height="58" rx="8"/>
<text class="title" x="576" y="210">colab-server</text>
<text class="small" x="576" y="230">anton, over Tailscale</text>
<path class="wire" d="M216 42H258V96H296" marker-end="url(#win-arrow)"/>
<path class="wire" d="M216 102H296" marker-end="url(#win-arrow)"/>
<path class="wire" d="M216 162H258V112H296" marker-end="url(#win-arrow)"/>
<path class="wire" d="M216 222H296" marker-end="url(#win-arrow)"/>
<path class="wire" d="M490 105H556" marker-end="url(#win-arrow)"/>
<path class="wire" d="M490 215H556" marker-end="url(#win-arrow)"/>
<path class="wire dash" d="M395 140V182"/>
<text class="small" x="402" y="166">SOCKS5 proxy from the local server</text>
</svg></div>

*Requests from a window go to the link of its host. Events from a link go only to windows of that host.*

### One notebook, one window

A registry in `windows.rs` maps each window to its notebook. When a window opens a notebook that another window of the same host already shows, the app focuses that other window. The second window stays as it was.

A new window first asks for its startup action, such as `open-recent:<path>`. An action for a window that still loads waits in the registry until its page asks. So a menu click during load is not lost.

### Where menu actions go

Open, Open Recent and Clone from Colab go to the focused window when it belongs to the right host. Otherwise they go to another window of that host, and a blank window comes first. With no window of that host, the app creates one.

When an agent opens a notebook and **Open notebooks that agents use** is on, the same rules apply. The app reuses a blank window or creates one. When the agent closes the notebook, the app closes its window.

### Window style

Notebook windows open at 1200 by 850 points, with a minimum of 700 by 500. Settings opens at 880 by 640. All windows are transparent with an overlay title bar and no title text. The UI draws its own title bar under the traffic lights. When the translucent setting is on, the UI asks Tauri for the Liquid Glass and sidebar effects. See [Translucent window](appearance.md#translucent-window).

## Menus

| Menu | Items |
|---|---|
| Colab Notebook | About, Settings... (<kbd>⌘</kbd><kbd>,</kbd>), Services, Hide, Hide Others, Show All, Quit |
| File | New Window (<kbd>⌘</kbd><kbd>⇧</kbd><kbd>N</kbd>), Connect to Host, Open... (<kbd>⌘</kbd><kbd>O</kbd>), Open Recent, Clone from Colab..., Close Window |
| Edit | Undo, Redo, Cut, Copy, Paste, Select All |
| View | Enter Full Screen |
| Window | Minimize, Zoom, Bring All to Front |

Open Recent keeps ten entries in `recent.json`. Each entry has a host and a path, and an entry of another host shows the host in brackets. The app builds the menu again when a window gains focus, so the host list and the recent list stay current.

## Menu bar icon

The menu bar icon stays while the app runs, also with no window open. Its menu is native AppKit code in `tray.rs`:

- A header with the server state: Running, Reconnecting or Stopped. A click opens **Settings > Server**.
- The app and server versions, with a button that copies both. When the server is older than the app, the line shows both versions.
- **Settings...** and **Quit Colab Notebook**.

When you close the last window, the app keeps running in the menu bar. A click on the Dock icon then opens a new window.

## Server lifecycle

The app never hosts the engine in its own process. At start it connects to the socket of the local server. When nothing answers, it runs its own binary as `colab-app server run` in a new session, with output to `server.log`. It does not try again within 10 seconds.

Quit stops the app. The server and its kernels keep running. **Settings > Server** has Start Server, Restart Server and Stop Server. After Stop Server the app does not start the server again until you press Start Server.

### Server updates

After each connect, the app asks the local server for its version. When the server is older than the app, it must restart, but not in the middle of your work:

1. The app asks `can_restart`. That is true only when no kernel is alive, local or hosted for another machine.
1. Until then, a notice in each window says: "The Colab Notebook server still runs version X. It restarts with the new version when no kernel is running."
1. The app asks again every 10 seconds. When the answer is true, it sends `shutdown`, and the link starts the new server.

The app never restarts a server that is newer than itself.

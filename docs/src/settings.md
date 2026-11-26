# Settings

This page lists every setting in the Settings window, section by section. The key column names the field in `ui/src/lib/settings.ts`.

Editor and appearance settings are saved in the web storage of the app under the key `settings`. Every window reads them and updates at once when one changes. Other settings live in their own files, as the tables say. See [Files and locations](files.md).

## Server

The Server section shows the state of the local server and its version.

| Setting | Description | Stored in |
|---|---|---|
| Start Server | Starts the local server. Shown when it is stopped. | none |
| Restart Server | Stops the server. The app then starts it again. | none |
| Stop Server | Stops the server. The app does not start it again until Start Server. | none |
| Tailscale | Turns the Tailscale device of this server on or off. Off by default. | `tailscale.json` |
| Device name | How this Mac appears on your tailnet. Editable while Tailscale is off. Default: short host name plus `-colab`. | `tailscale.json` |
| Sign in to Tailscale | Opens the Tailscale sign in page. Shown while the device waits for you. | Tailscale state folder |
| Account | The Tailscale login, with Sign out. | Tailscale state folder |
| DNS name | The full MagicDNS name of the device. Read only. | none |

## Appearance

| Setting | Key | Default | Values |
|---|---|---|---|
| Appearance | `appearance` | Dark | Light, Dark, Auto |
| Accent color | `accent` | Theme | Theme, eight swatches, or any color as `#rrggbb` |
| Theme, dark | `theme` | Default | 13 dark themes |
| Theme, light | `lightTheme` | GitHub Light | 4 light themes |
| Translucent window | `glass` | On | On, off. Mac only. |
| Content opacity | `opacity` | 100% | 60% to 100%. Shown when the window is translucent. |
| Notebook padding | `padding` | 12 px | 0 to 48 px, steps of 4 |
| Corner radius | `radius` | 8 px | 0 to 16 px |
| Interface font | `interfaceFont`, `interfaceSize` | Inter, 13 | Any installed font, size 10 to 20 |
| Markdown font | `markdownFont`, `markdownSize` | Inter, 14 | Any installed font, size 10 to 28 |
| Code font | `codeFont`, `codeSize` | JetBrains Mono, 13 | Installed monospace fonts, size 8 to 32 |

The [Appearance and themes](appearance.md) page explains these in depth.

## Editor

| Setting | Key | Default | Description |
|---|---|---|---|
| Cursor style | `cursorStyle` | Bar | Bar, Block or Underline. |
| Cursor blink | `cursorBlink` | On | Makes the cursor blink. |
| Line height | `lineHeight` | 1.6 | 1 to 3, steps of 0.1. |
| Tab size | `tabSize` | 4 | 1 to 8 spaces. Also the indent unit. |
| Font ligatures | `ligatures` | Off | Turns on ligatures of the code font. |
| Line numbers | `lineNumbers` | Off | Shows line numbers in code cells. |
| Word wrap | `wordWrap` | On | Wraps long lines in cells. |
| Lint warnings | `lintDiagnostics` | On | Shows ruff diagnostics, with your project's ruff config. |
| Type errors | `typeDiagnostics` | On | Shows ty diagnostics. Missing imports are hidden on Colab kernels. |
| Double click to edit markdown | `dblclickMarkdown` | Off | A double click opens the editor of a markdown cell. Enter and the pencil always work. |
| Open notebooks that agents use | `open_agent_notebooks` in `app.json` | Off | Shows a window when an agent opens a notebook, and closes it when the agent is done. |

## Keyboard

Click a shortcut, then press the new key combination. <kbd>Esc</kbd> cancels. A changed shortcut shows a **Reset** button. The keys are stored under `keys`. The [Keyboard shortcuts](keyboard-shortcuts.md) page lists the actions and defaults.

## Account

| Setting | Description | Stored in |
|---|---|---|
| Sign in with Google | Signs in to Colab in your browser. Then shows the email and **Sign out**. | Keychain, service `colab-app`, account `google-oauth` |
| Client id | Optional. Your own desktop OAuth client, if the built in one stops working. | `clientId` in web storage |
| Client secret | The secret of that client. | `clientSecret` in web storage |

Leave both client fields empty to use the default client. When Google rejects a client, the section shows the error and asks for your own client.

## Remote Hosts

Each saved host shows its name, address, kernel type and state. The info button shows the address, the path (Direct, Relay or None), the latency and a **Remove** button.

**Add Remote Host...** asks for a **Name** and an **Address**. The list is stored in `hosts.json` and shared with `colab-server hosts`. See [Add a host](remote-hosts.md#add-a-host).

## Secrets

Each row has a name, a hidden value, **Show** and **Delete**. **Add new secret** adds a row. All values are stored in one Keychain item. See [Secrets](secrets.md).

## About

Shows the version of the app.

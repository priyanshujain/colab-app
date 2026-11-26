# Keyboard Shortcuts

The app copies the Jupyter model of edit mode and command mode. Nine actions have shortcuts that you can change. The rest are fixed.

## Actions you can change

Change these in **Settings > Keyboard**. Click a shortcut and press the new keys. In `settings.ts`, `Mod` means <kbd>⌘</kbd>, and <kbd>Ctrl</kbd> also counts as `Mod`.

| Action | Default | Works in |
|---|---|---|
| Run cell | <kbd>⌘</kbd><kbd>↩</kbd> | Edit mode and command mode. Runs the cell and keeps it selected. |
| Run all | <kbd>⌘</kbd><kbd>⇧</kbd><kbd>↩</kbd> | Anywhere in the notebook window. |
| Find | <kbd>⌘</kbd><kbd>F</kbd> | Anywhere in the notebook window. |
| Format cell | <kbd>⌥</kbd><kbd>⇧</kbd><kbd>F</kbd> | Edit mode and command mode. |
| Format notebook | <kbd>⌘</kbd><kbd>⌥</kbd><kbd>⇧</kbd><kbd>F</kbd> | Anywhere in the notebook window. |
| Organize imports | <kbd>⌥</kbd><kbd>⇧</kbd><kbd>O</kbd> | Edit mode for the cell, command mode for the notebook. |
| Rename symbol | <kbd>⌘</kbd><kbd>R</kbd> | Edit mode, on a name. |
| Go to definition | <kbd>F12</kbd> | Edit mode, on a name. |
| Quick fix | <kbd>⌘</kbd><kbd>.</kbd> | Edit mode, on a diagnostic. |

## Edit mode

| Keys | Action |
|---|---|
| <kbd>⇧</kbd><kbd>↩</kbd> | Run the cell and move to the next one. In the last cell, add a new code cell. |
| <kbd>Esc</kbd> | Leave the editor and go to command mode. |
| <kbd>⌘</kbd> click | Go to the definition of the name under the mouse. |
| <kbd>Tab</kbd> | Indent the line or the selection. |
| <kbd>↩</kbd> in a completion list | Accept the completion. |

The editor is CodeMirror 6. Its standard keys for undo, selection, brackets and completion also work.

## Command mode

These single keys work when the cell list has focus and no editor is open.

| Keys | Action |
|---|---|
| <kbd>↩</kbd> | Edit the selected cell. |
| <kbd>⇧</kbd><kbd>↩</kbd> | Run the cell and select the next one. |
| <kbd>↓</kbd> or <kbd>J</kbd> | Select the next cell. |
| <kbd>↑</kbd> or <kbd>K</kbd> | Select the previous cell. |
| <kbd>A</kbd> | Add a code cell above. |
| <kbd>B</kbd> | Add a code cell below. |
| <kbd>M</kbd> | Change the cell to markdown. |
| <kbd>Y</kbd> | Change the cell to code. |
| <kbd>D</kbd> <kbd>D</kbd> | Delete the cell. |

## Find bar

| Keys | Action |
|---|---|
| <kbd>↩</kbd> | Next match. |
| <kbd>⇧</kbd><kbd>↩</kbd> | Previous match. |
| <kbd>Esc</kbd> | Close the find bar. |

## Menus

| Keys | Action |
|---|---|
| <kbd>⌘</kbd><kbd>O</kbd> | Open a notebook. |
| <kbd>⌘</kbd><kbd>⇧</kbd><kbd>N</kbd> | New window. |
| <kbd>⌘</kbd><kbd>,</kbd> | Open Settings. |
| <kbd>⌘</kbd><kbd>W</kbd> | Close the window. |
| <kbd>⌘</kbd><kbd>Q</kbd> | Quit the app. The server and its kernels keep running. |

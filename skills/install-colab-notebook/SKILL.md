---
name: install-colab-notebook
description: Install the Colab Notebook Mac app from its latest GitHub release, register its MCP server with Claude Code and install the colab-notebook skill. Use when asked to install or set up Colab Notebook.
---

Run each step in order and stop with a clear message if one fails. Never delete notebooks, settings or anything in `~/Library/Application Support`.

- Check the Mac: `sysctl -n hw.optional.arm64` must print `1`. Otherwise stop: the app needs Apple Silicon.
- Pick the folder: `dest=/Applications`, or `dest="$HOME/Applications"` (create it) when `/Applications` is not writable.
- If `"$dest/Colab Notebook.app"` exists, ask the user before you replace it. If they say no, skip to the MCP step. If yes, quit it with `osascript -e 'quit app "Colab Notebook"'` and remove only that `.app` bundle.
- Download the DMG into a new temp folder, `tmp=$(mktemp -d)`:
  - With `gh`: `gh release download --repo priyanshujain/colab-app --pattern '*-aarch64.dmg' --dir "$tmp"`
  - Without: `curl -fsSL -o "$tmp/colab.dmg" "$(curl -fsSL https://api.github.com/repos/priyanshujain/colab-app/releases/latest | grep -o 'https://[^"]*-aarch64\.dmg' | head -1)"`
- Install: `hdiutil attach -nobrowse -readonly -mountpoint "$tmp/mnt" "$tmp"/*.dmg`, then `ditto "$tmp/mnt/Colab Notebook.app" "$dest/Colab Notebook.app"`, then `hdiutil detach "$tmp/mnt"`.
- Verify: `spctl -a -vv "$dest/Colab Notebook.app"` must print `source=Notarized Developer ID`. If not, stop and tell the user.
- Register MCP, unless `claude mcp get colab` already finds it: `claude mcp add --scope user colab -- "$dest/Colab Notebook.app/Contents/MacOS/colab-app" mcp`
- Install the notebook skill: `mkdir -p ~/.claude/skills/colab-notebook && curl -fsSL -o ~/.claude/skills/colab-notebook/SKILL.md https://raw.githubusercontent.com/priyanshujain/colab-app/main/skills/colab-notebook/SKILL.md`
- Launch: `open "$dest/Colab Notebook.app"`. Remove `$tmp` when done.
- Tell the user what is left: start a new Claude Code session so it loads the MCP server and the skill; sign in with Google in Settings > Account to use Colab runtimes; turn on Tailscale in Settings > Server only to use other machines. Docs: https://priyanshujain.github.io/colab-app/

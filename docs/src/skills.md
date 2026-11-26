# Skills

A skill is a Markdown file that teaches an agent a task. The repository has two skills in `skills/`. Claude Code reads each one from `~/.claude/skills/<name>/SKILL.md`.

| Skill | What it does |
|---|---|
| `install-colab-notebook` | Installs the app from the latest release, registers the MCP server and installs `colab-notebook`. |
| `colab-notebook` | Teaches the agent to open, edit and run notebooks through the app, the Colab way. |

## Let the agent install everything

Paste this prompt into Claude Code on your Mac:

```text
Fetch https://raw.githubusercontent.com/priyanshujain/colab-app/main/skills/install-colab-notebook/SKILL.md and follow it to install Colab Notebook on this Mac.
```

The agent asks before it replaces an existing install. It never deletes your notebooks or settings.

## Install the skills yourself

Each command fetches the file from GitHub into place.

The notebook skill:

```sh
mkdir -p ~/.claude/skills/colab-notebook && curl -fsSL -o ~/.claude/skills/colab-notebook/SKILL.md https://raw.githubusercontent.com/priyanshujain/colab-app/main/skills/colab-notebook/SKILL.md
```

The install skill:

```sh
mkdir -p ~/.claude/skills/install-colab-notebook && curl -fsSL -o ~/.claude/skills/install-colab-notebook/SKILL.md https://raw.githubusercontent.com/priyanshujain/colab-app/main/skills/install-colab-notebook/SKILL.md
```

The notebook skill needs the MCP server. Register it once:

```sh
claude mcp add --scope user colab -- "/Applications/Colab Notebook.app/Contents/MacOS/colab-app" mcp
```

Start a new Claude Code session after these steps, so it loads the server and the skills. [Agents](agents-mcp.md) explains the tools that the skill uses.

## install-colab-notebook

The full text of `skills/install-colab-notebook/SKILL.md`:

```text
{{#include ../../skills/install-colab-notebook/SKILL.md}}
```

## colab-notebook

The full text of `skills/colab-notebook/SKILL.md`:

```text
{{#include ../../skills/colab-notebook/SKILL.md}}
```

The page includes both files at build time, so the text here always matches the repository.

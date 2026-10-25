# Colab Notebook

A macOS desktop app for Jupyter notebooks that run on Google Colab runtimes or a local `.venv`.

The app runs an MCP server at `http://127.0.0.1:7761/mcp`, so Claude Code can open notebooks, connect a Colab runtime, and edit and run cells while you watch them live in the app. Register it with `claude mcp add --transport http --scope user colab http://127.0.0.1:7761/mcp` and install the skill in [skills/colab-notebook](skills/colab-notebook/SKILL.md) by copying it to `~/.claude/skills/`.

Build and install with `just install`, run tests with `just test`.

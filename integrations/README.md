# AI agent integrations

Two ways to give an AI coding agent access to your MemoryWhale memory.

## 1. MCP server (recommended)

`mw-mcp` is a [Model Context Protocol](https://modelcontextprotocol.io) server
over stdio. Register it once and the agent can query your memory with native
tools (`recent_errors`, `search_memory`, `get_context`) — no copy-paste.

Claude Code:

```bash
claude mcp add memorywhale -- mw-mcp
```

Codex / Cursor / other MCP clients: add a stdio server whose command is `mw-mcp`
(no arguments). It honours `MEMORYWHALE_DATA_DIR` like the rest of the CLI.

Quick check that it responds:

```bash
printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | mw-mcp
```

## 2. Claude Code skill

[`claude-code/memorywhale/SKILL.md`](claude-code/memorywhale/SKILL.md) teaches
Claude Code *when* to reach for the memory (recurring failures, "how did we fix
this last time"). Install it by copying (or symlinking) the folder into your
skills directory:

```bash
cp -r integrations/claude-code/memorywhale ~/.claude/skills/
```

The skill uses the MCP tools when connected and falls back to the `mw context`
CLI otherwise, so it's useful with or without step 1.

## Without either

`mw context` prints a compact, paste-ready digest for any agent or chat:

```bash
mw context --last-error
```

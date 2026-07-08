# Cursor integration

Give Cursor's AI the same access to your MemoryWhale memory that Claude Code
gets: it can read what already failed and write down what it figured out. Two
pieces — an MCP server and a Rule.

## 1. Connect the MCP server

`mw-mcp` is a Model Context Protocol server (`recent_errors`, `search_memory`,
`get_context`, `remember`). Point Cursor at it via `mcp.json`:

- **This project only:** copy [`mcp.json`](mcp.json) to `.cursor/mcp.json` in
  your project root.
- **Every project:** put the same JSON in `~/.cursor/mcp.json`.

```json
{
  "mcpServers": {
    "memorywhale": {
      "command": "mw-mcp"
    }
  }
}
```

`mw-mcp` must be on `PATH` (the standard MemoryWhale install). If it isn't, use
its absolute path as `command`. To point at a non-default database, add
`"env": { "MEMORYWHALE_DATA_DIR": "/path/to/dir" }`.

Then in Cursor: **Settings → MCP** should show `memorywhale` with its four tools
enabled.

## 2. Add the Rule (the "when to use it" part)

The MCP server gives Cursor the *tools*; this Rule teaches it *when* to reach
for them (recurring failures, "how did we fix this last time", and saving a
conclusion once a fix is found).

- **This project only:** copy [`memorywhale.mdc`](memorywhale.mdc) to
  `.cursor/rules/memorywhale.mdc`.
- **Every project:** add it via Cursor **Settings → Rules → User Rules**.

It's an "Agent Requested" rule (`alwaysApply: false`), so Cursor pulls it in
only when the description matches what you're doing — no wasted context on
unrelated work.

## Without the MCP server

The Rule falls back to the `mw` CLI, so even with only the binaries installed,
Cursor can run `mw context --last-error` / `mw search "…"` / `mw remember "…"`.
And with no editor integration at all, `mw ask` packages the last failure onto
your clipboard for any chat.

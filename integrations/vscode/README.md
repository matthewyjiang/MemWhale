# VS Code (GitHub Copilot) integration

Copilot's **agent mode** supports MCP servers, so it can query your MemoryWhale
memory with the same four tools Claude Code and Cursor get: `recent_errors`,
`search_memory`, `get_context`, `remember`.

## Connect the MCP server

Copy [`mcp.json`](mcp.json) to **`.vscode/mcp.json`** in your workspace (or add
the same block to your user `settings.json` under `"mcp"`):

```json
{
  "servers": {
    "memorywhale": {
      "type": "stdio",
      "command": "mw-mcp"
    }
  }
}
```

`mw-mcp` must be on `PATH` (the standard MemoryWhale install); otherwise use its
absolute path. To point at a non-default database, add
`"env": { "MEMORYWHALE_DATA_DIR": "/path/to/dir" }`.

Open the Copilot Chat view, switch to **Agent** mode, and MemoryWhale's tools
appear in the tools picker. (VS Code's exact MCP config key has shifted across
versions — if `"servers"` isn't recognized, check your version's "MCP servers"
docs; the `command: mw-mcp` part is the same everywhere.)

## Tell it when to reach for the memory

Add this to `.github/copilot-instructions.md` (repo-wide) so Copilot uses the
tools proactively:

> When a build/test/deploy fails, before proposing a fix, call `search_memory`
> or `recent_errors` to check whether this failure has a known cause or a saved
> lesson. Once you've figured out *why* something failed or *how* a fix worked,
> call `remember` to save that conclusion for next time.

Without the MCP server, the `mw` CLI still works: `mw context --last-error`,
`mw search "…"`, `mw remember "…"`.

# Windsurf integration

Windsurf (Cascade) supports MCP servers, so it gets MemoryWhale's four tools:
`recent_errors`, `search_memory`, `get_context`, `remember`.

## Connect the MCP server

Merge [`mcp_config.json`](mcp_config.json) into Windsurf's MCP config at
**`~/.codeium/windsurf/mcp_config.json`** (or use the **Cascade → MCP →
Configure / "Add Server"** UI, which edits the same file):

```json
{
  "mcpServers": {
    "memorywhale": {
      "command": "mw-mcp"
    }
  }
}
```

`mw-mcp` must be on `PATH` (standard MemoryWhale install); otherwise use its
absolute path. For a non-default database add
`"env": { "MEMORYWHALE_DATA_DIR": "/path/to/dir" }`. After saving, hit the
refresh/reload in the MCP panel so Cascade picks up the server.

## Tell it when to reach for the memory

Add a Windsurf Rule (**Customizations → Rules**, or a `.windsurf/rules/`
file):

> When a build/test/deploy fails, before proposing a fix, use `search_memory`
> or `recent_errors` to check whether this failure has a known cause or a saved
> lesson. Once you've figured out why something failed or how a fix worked, use
> `remember` to save that conclusion.

Without the MCP server, the `mw` CLI still works: `mw context --last-error`,
`mw search "…"`, `mw remember "…"`.

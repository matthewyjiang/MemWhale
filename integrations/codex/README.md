# Codex CLI integration

OpenAI's Codex CLI supports MCP servers via its TOML config, so it gets
MemoryWhale's four tools: `recent_errors`, `search_memory`, `get_context`,
`remember`.

## Connect the MCP server

Add the block from [`config.toml`](config.toml) to **`~/.codex/config.toml`**:

```toml
[mcp_servers.memorywhale]
command = "mw-mcp"
```

`mw-mcp` must be on `PATH` (standard MemoryWhale install); otherwise use its
absolute path. For a non-default database add
`env = { MEMORYWHALE_DATA_DIR = "/path/to/dir" }`. (Codex's MCP config key has
been `mcp_servers` — if a newer version differs, check `codex --help` / its
config docs; `command = "mw-mcp"` is the same regardless.)

## Tell it when to reach for the memory

Codex reads an `AGENTS.md` at your repo root. Add:

> When a build/test/deploy fails, before proposing a fix, use `search_memory`
> or `recent_errors` (MemoryWhale MCP) to check whether this failure has a known
> cause or a saved lesson. Once you've figured out why something failed or how a
> fix worked, use `remember` to save that conclusion.

Without the MCP server, the `mw` CLI still works: `mw context --last-error`,
`mw search "…"`, `mw remember "…"`.

# Codex CLI + MemoryWhale

OpenAI's Codex CLI supports MCP servers via TOML config, so it can use
MemoryWhale's six local memory tools.

## Status

Verified against OpenAI's [Codex MCP](https://developers.openai.com/codex/mcp)
documentation on 2026-08-29. User config is `~/.codex/config.toml`. Trusted
projects may also use `.codex/config.toml`. Each server is a
`[mcp_servers.<name>]` table with `command` for stdio. The TUI lists servers
with `/mcp`.

## Requirements

- MemoryWhale installed with `mw-mcp` on `PATH`.
- Codex CLI (or the Codex IDE extension, which opens the same `config.toml`).

## Setup

Add the block from [`config.toml`](config.toml) to `~/.codex/config.toml`:

```toml
[mcp_servers.memorywhale]
command = "mw-mcp"
```

`mw-mcp` must be on `PATH` (standard MemoryWhale install); otherwise use its
absolute path. For a non-default database add
`env = { MEMORYWHALE_DATA_DIR = "/path/to/dir" }`. The top-level key is
`mcp_servers`, not `mcpServers`.

### Memory-use guidance

Codex reads an `AGENTS.md` at your repo root. Add:

> When a build/test/deploy fails, before proposing a fix, use `search_memory`
> or `recent_errors` (MemoryWhale MCP) to check whether this failure has a known
> cause or a saved lesson. Once you've figured out why something failed or how a
> fix worked, use `remember` to save that conclusion.

## Verify

```bash
command -v mw-mcp
```

In the Codex TUI, run `/mcp` and confirm `memorywhale` is active. You should
see the six tools: `recent_errors`, `search_memory`, `get_context`, `remember`,
`similar_failures`, and `stats`.

## Available capabilities

| Capability | Available |
| --- | --- |
| MCP memory access | Yes |
| Automatic execution capture | No |
| Memory-use guidance | Yes, via `AGENTS.md` |

MCP access is not automatic execution capture. Without the MCP server, the
`mw` CLI still works: `mw context --last-error`, `mw search "…"`,
`mw remember "…"`.

## Example prompt

> Use MemoryWhale to check whether I encountered a similar failure before.

## Troubleshooting

- Run `command -v mw-mcp` from the environment that launches Codex.
- Confirm you edited `[mcp_servers.memorywhale]`, not an `mcpServers` JSON
  block from another client.
- Restart Codex after editing `config.toml`.
- If the wrong database opens, set `MEMORYWHALE_DATA_DIR` in `env`.
- Run `mw doctor` to check the MemoryWhale install.

## Uninstall

Delete the `[mcp_servers.memorywhale]` table from `~/.codex/config.toml` (and
any project `.codex/config.toml`). Remove the `AGENTS.md` instruction if you
added one. This does not delete the MemoryWhale database.

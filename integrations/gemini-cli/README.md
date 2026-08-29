# Gemini CLI + MemoryWhale

Google's [Gemini CLI](https://github.com/google-gemini/gemini-cli) connects to
MCP servers, so it can use MemoryWhale's six local memory tools.

## Status

Verified against Gemini CLI's
[MCP setup tutorial](https://geminicli.com/docs/cli/tutorials/mcp-setup/)
and the project's
[MCP server docs](https://github.com/google-gemini/gemini-cli/blob/main/docs/tools/mcp-server.md)
on 2026-08-29. Servers go in `mcpServers` inside `~/.gemini/settings.json` or
`.gemini/settings.json`. Stdio servers use `command`. `/mcp list` shows
connection status.

## Requirements

- MemoryWhale installed with `mw-mcp` on `PATH`.
- Gemini CLI installed.

## Setup

Add the `memorywhale` entry from [`settings.json`](settings.json) to your Gemini
CLI settings:

- Every project: `~/.gemini/settings.json`
- This project only: `.gemini/settings.json` in the project root

```json
{
  "mcpServers": {
    "memorywhale": {
      "command": "mw-mcp"
    }
  }
}
```

If the file already has an `mcpServers` object, add `memorywhale` alongside your
other servers rather than replacing it. `mw-mcp` must be on `PATH` (the standard
MemoryWhale install); otherwise use its absolute path as `command`. For a
non-default database add `"env": { "MEMORYWHALE_DATA_DIR": "/path/to/dir" }`.

### Memory-use guidance

A `GEMINI.md` instruction (in your project or `~/.gemini/`) teaches the CLI
when to use the tools:

> When a build/test/deploy fails, before proposing a fix, use `search_memory`
> or `recent_errors` (MemoryWhale MCP) to check whether this failure has a known
> cause or a saved lesson. Once you've figured out why something failed or how a
> fix worked, use `remember` to save that conclusion.

## Verify

```bash
command -v mw-mcp
```

Restart Gemini CLI and run `/mcp list`. `memorywhale` should show as connected
with the six tools: `recent_errors`, `search_memory`, `get_context`,
`remember`, `similar_failures`, and `stats`.

## Available capabilities

| Capability | Available |
| --- | --- |
| MCP memory access | Yes |
| Automatic execution capture | No |
| Memory-use guidance | Yes, via `GEMINI.md` |

MCP access is not automatic execution capture. Without the MCP server, the
`mw` CLI still works: `mw context --last-error`, `mw search "…"`,
`mw remember "…"`.

## Example prompt

> Use MemoryWhale to check whether I encountered a similar failure before.

## Troubleshooting

- Run `command -v mw-mcp` from the environment that launches Gemini CLI.
- Confirm `mcpServers` is valid JSON and restart the CLI.
- If `/mcp list` shows Disconnected, the binary is missing from `PATH`.
- If the wrong database opens, set `MEMORYWHALE_DATA_DIR` in `env`.
- Run `mw doctor` to check the MemoryWhale install.

## Uninstall

Delete the `memorywhale` entry from `mcpServers` in
`~/.gemini/settings.json` and/or `.gemini/settings.json`. Remove any
`GEMINI.md` instruction you added. Restart Gemini CLI. This does not delete
the MemoryWhale database.

# Windsurf + MemoryWhale

Windsurf Cascade supports MCP servers, so it can use MemoryWhale's six local
memory tools.

## Status

Verified against Windsurf's
[Cascade MCP](https://docs.windsurf.com/plugins/cascade/mcp) documentation on
2026-08-29. That page documents editing `mcp_config.json` under
`~/.codeium/mcp_config.json`, with a `mcpServers` object and a `command` for
stdio servers. Add servers from Settings → Tools → Windsurf Settings → Add
Server, or via View Raw Config. Press refresh after saving.

Some Windsurf UI builds still open `~/.codeium/windsurf/mcp_config.json`. Use
the file the UI opens. The `mcpServers` / `command` schema is the same.

## Requirements

- MemoryWhale installed with `mw-mcp` on `PATH`.
- Windsurf with Cascade MCP enabled.

## Setup

Merge [`mcp_config.json`](mcp_config.json) into the MCP config file the UI
opens, or add the server through Cascade → MCP → Add Server:

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
`"env": { "MEMORYWHALE_DATA_DIR": "/path/to/dir" }`. After saving, hit refresh
in the MCP panel so Cascade picks up the server.

### Memory-use guidance

Add a Windsurf Rule (Customizations → Rules, or a `.windsurf/rules/`
file):

> When a build/test/deploy fails, before proposing a fix, use `search_memory`
> or `recent_errors` to check whether this failure has a known cause or a saved
> lesson. Once you've figured out why something failed or how a fix worked, use
> `remember` to save that conclusion.

## Verify

```bash
command -v mw-mcp
```

Refresh the MCP panel and confirm `memorywhale` is active with the six tools:
`recent_errors`, `search_memory`, `get_context`, `remember`,
`similar_failures`, and `stats`.

## Available capabilities

| Capability | Available |
| --- | --- |
| MCP memory access | Yes |
| Automatic execution capture | No |
| Memory-use guidance | Yes, optional Rule |

MCP access is not automatic execution capture. Without the MCP server, the
`mw` CLI still works: `mw context --last-error`, `mw search "…"`,
`mw remember "…"`.

## Example prompt

> Use MemoryWhale to check whether I encountered a similar failure before.

## Troubleshooting

- Run `command -v mw-mcp` in the environment that launches Windsurf. Use an
  absolute `command` if needed.
- Edit the config file the UI actually opened, then press refresh.
- If the wrong database opens, set `MEMORYWHALE_DATA_DIR` in the server `env`.
- Run `mw doctor` to check the MemoryWhale install.

## Uninstall

Delete the `memorywhale` entry from `mcp_config.json` and refresh or restart
Cascade. Remove any Windsurf Rule you added. This does not delete the
MemoryWhale database.

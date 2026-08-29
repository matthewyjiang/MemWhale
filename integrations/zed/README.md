# Zed + MemoryWhale

Zed supports custom MCP servers, so its Agent Panel can use MemoryWhale's six
local memory tools.

## Status

Verified against Zed's [Model Context Protocol](https://zed.dev/docs/ai/mcp)
documentation on 2026-08-29. Custom servers are added from Settings → AI →
MCP Servers (or `agent: open settings`), and they write `context_servers`
entries in the settings file. Local servers use `command`, `args`, and `env`.

## Requirements

- MemoryWhale installed with `mw-mcp` on `PATH`.
- Zed with the Agent Panel.

## Setup

Open Settings → AI → MCP Servers and choose Add Server → Add Local
Server, or run `agent: open settings` to stay in that UI. To edit the JSON
settings file, run `zed: open settings file` and merge the
[`settings.json`](settings.json) example into your existing settings:

```json
{
  "context_servers": {
    "memorywhale": {
      "command": "mw-mcp",
      "args": [],
      "env": {}
    }
  }
}
```

`mw-mcp` must be on `PATH` (standard MemoryWhale install); otherwise use its
absolute path. For a non-default database, add
`"env": { "MEMORYWHALE_DATA_DIR": "/path/to/dir" }`.

### Memory-use guidance

Add this to your project or user rules:

> When a build, test, or deploy fails, use `search_memory` or `recent_errors`
> before proposing a fix. After finding the cause or a working fix, use
> `remember` to save the conclusion.

## Verify

```bash
command -v mw-mcp
```

After saving, open the Agent Panel settings and confirm that `memorywhale` has
a green running indicator. The six tools are `recent_errors`, `search_memory`,
`get_context`, `remember`, `similar_failures`, and `stats`.

## Available capabilities

| Capability | Available |
| --- | --- |
| MCP memory access | Yes |
| Automatic execution capture | No |
| Memory-use guidance | Yes, optional rules |

MCP access is not automatic execution capture. Without the MCP server, the CLI
remains available: `mw context --last-error`, `mw search "…"`, and
`mw remember "…"`.

## Example prompt

> Use MemoryWhale to check whether I encountered a similar failure before.

## Troubleshooting

- Run `command -v mw-mcp` from the environment Zed launches from.
- Confirm the settings key is `context_servers`, not `mcpServers`.
- If the server stays red, open `zed: open log` and look for context-server
  errors.
- If the wrong database opens, set `MEMORYWHALE_DATA_DIR` in `env`.
- Run `mw doctor` to check the MemoryWhale install.

## Uninstall

Delete the `memorywhale` entry from `context_servers` in Zed settings. Remove
any rule you added. This does not delete the MemoryWhale database.

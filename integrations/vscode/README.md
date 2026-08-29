# VS Code (GitHub Copilot) + MemoryWhale

Copilot's agent mode supports MCP servers, so it can query your MemoryWhale
store with the six `mw-mcp` tools.

## Status

Verified against VS Code's
[Add and manage MCP servers](https://code.visualstudio.com/docs/copilot/customization/mcp-servers)
and [MCP configuration reference](https://code.visualstudio.com/docs/copilot/reference/mcp-configuration)
on 2026-08-29. Workspace config is `.vscode/mcp.json` under a `servers` object.
A `"type": "stdio"` field is documented for local servers.

## Requirements

- MemoryWhale installed with `mw-mcp` on `PATH`.
- VS Code with GitHub Copilot Chat, using Agent mode.
- A model that can call tools.

## Setup

Copy [`mcp.json`](mcp.json) to `.vscode/mcp.json` in your workspace (or add
the same block through **MCP: Open User Configuration** for a user-level
server):

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

### Memory-use guidance

Add this to `.github/copilot-instructions.md` (repo-wide) so Copilot uses the
tools when debugging:

> When a build/test/deploy fails, before proposing a fix, call `search_memory`
> or `recent_errors` to check whether this failure has a known cause or a saved
> lesson. Once you've figured out why something failed or how a fix worked,
> call `remember` to save that conclusion for next time.

## Verify

```bash
command -v mw-mcp
```

Open `.vscode/mcp.json` and use the CodeLens Start control on the `memorywhale`
server if VS Code shows one. Then open Copilot Chat, switch to Agent mode, and
confirm the six MemoryWhale tools in the tools picker:
`recent_errors`, `search_memory`, `get_context`, `remember`,
`similar_failures`, and `stats`.

## Available capabilities

| Capability | Available |
| --- | --- |
| MCP memory access | Yes |
| Automatic execution capture | No |
| Memory-use guidance | Yes, via Copilot instructions |

MCP access is not automatic execution capture. Without the MCP server, the
`mw` CLI still works: `mw context --last-error`, `mw search "…"`,
`mw remember "…"`.

## Example prompt

> Use MemoryWhale to check whether I encountered a similar failure before.

## Troubleshooting

- Run `command -v mw-mcp` from the same environment VS Code uses. Remote
  windows need MemoryWhale on the remote, or a workspace MCP config that
  runs there.
- Confirm the file uses `"servers"` (VS Code), not `"mcpServers"`.
- Reload the window after editing `mcp.json`.
- If the wrong database opens, set `MEMORYWHALE_DATA_DIR` in the server `env`.
- Run `mw doctor` to check the MemoryWhale install.

## Uninstall

Delete the `memorywhale` entry from `.vscode/mcp.json` or the user MCP
configuration. Remove any Copilot instruction you added. This does not delete
the MemoryWhale database.

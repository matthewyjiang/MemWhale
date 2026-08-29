# Cline + MemoryWhale

[Cline](https://github.com/cline/cline) supports local stdio MCP servers, so it
can use MemoryWhale's six local memory tools.

## Status

Verified against Cline's
[MCP overview](https://docs.cline.bot/mcp/mcp-overview) and
[configuring MCP servers](https://docs.cline.bot/mcp/configuring-mcp-servers)
documentation on 2026-08-29. The IDE opens MCP settings from the Cline panel
(MCP Servers → Configure MCP Servers); that file lives under the editor's
globalStorage, not under `~/.cline/`. CLI config is
`~/.cline/data/settings/cline_mcp_settings.json`. Cline's MCP page still lists
`~/.cline/mcp.json`; the CLI does not read that file. Local servers use
`command` under a `mcpServers` object.

## Requirements

- MemoryWhale installed with `mw-mcp` on `PATH`.
- Cline (VS Code/compatible extension or Cline CLI).

## Setup

### IDE

In the editor, click the Cline icon → MCP Servers → Configure MCP Servers.
That opens Cline's MCP settings JSON. Add the `memorywhale` entry from
[`mcp_settings.json`](mcp_settings.json):

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
other servers rather than replacing it. Prefer the Cline UI; it points at the
file this install actually uses.

Typical VS Code global-storage locations, if you need to find the file by
hand:

```text
~/Library/Application Support/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json   # macOS
~/.config/Code/User/globalStorage/saoudrizwan.claude-dev/settings/cline_mcp_settings.json                       # Linux
%APPDATA%\Code\User\globalStorage\saoudrizwan.claude-dev\settings\cline_mcp_settings.json                        # Windows
```

### CLI

```bash
cline mcp add memorywhale -- mw-mcp
```

That writes `~/.cline/data/settings/cline_mcp_settings.json`.

`mw-mcp` must be on `PATH` (the standard MemoryWhale install); otherwise use its
absolute path as `command`. For a non-default database add
`"env": { "MEMORYWHALE_DATA_DIR": "/path/to/dir" }`.

### Memory-use guidance

Add to a `.clinerules` file (or Cline's custom instructions):

> When a build/test/deploy fails, before proposing a fix, use `search_memory`
> or `recent_errors` (MemoryWhale MCP) to check whether this failure has a known
> cause or a saved lesson. Once you've figured out why something failed or how a
> fix worked, use `remember` to save that conclusion.

## Verify

```bash
command -v mw-mcp
```

On Windows PowerShell use `Get-Command mw-mcp`; in Command Prompt use
`where.exe mw-mcp`.

In the Cline MCP panel, confirm `memorywhale` is enabled and lists the six
tools: `recent_errors`, `search_memory`, `get_context`, `remember`,
`similar_failures`, and `stats`. From the CLI, `cline mcp` can list servers.

## Available capabilities

| Capability | Available |
| --- | --- |
| MCP memory access | Yes |
| Automatic execution capture | No |
| Memory-use guidance | Yes, via `.clinerules` |

MCP access is not automatic execution capture. Without the MCP server, the
`mw` CLI still works: `mw context --last-error`, `mw search "…"`,
`mw remember "…"`.

## Example prompt

> Use MemoryWhale to check whether I encountered a similar failure before.

## Troubleshooting

- Confirm `mw-mcp` is on `PATH` from the environment that launches Cline
  (`command -v mw-mcp`, or `Get-Command` / `where.exe` on Windows).
- Edit the file the Cline UI opened, not a guessed path.
- Restart Cline after changing MCP settings if tools do not appear.
- If the wrong database opens, set `MEMORYWHALE_DATA_DIR` in `env`.
- Run `mw doctor` to check the MemoryWhale install.

## Uninstall

Delete the `memorywhale` entry from Cline's MCP settings (IDE Configure MCP
Servers, or `cline mcp` delete). Remove any `.clinerules` instruction you
added. This does not delete the MemoryWhale database.

# Claude Desktop + MemoryWhale

[Claude Desktop](https://claude.ai/download) is Anthropic's desktop app and an
MCP host. Point it at MemoryWhale's MCP server to use the local memory tools.
This is the desktop chat app. For the Claude Code CLI, see
[`../claude-code/`](../claude-code/).

## Status

Verified against the Model Context Protocol
[local stdio server guide](https://modelcontextprotocol.io/docs/develop/connect-local-servers)
on 2026-08-29. That page documents the macOS and Windows config paths below.
Claude Desktop reads MCP config at launch.

## Requirements

- MemoryWhale installed with `mw-mcp` on `PATH`.
- Claude Desktop installed.
- A full quit and reopen after editing the config file.

## Setup

Edit Claude Desktop's config file. Anthropic's local-server documentation lists:

- macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
- Windows: `%APPDATA%\Claude\claude_desktop_config.json`

Add the `memorywhale` entry from
[`claude_desktop_config.json`](claude_desktop_config.json):

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
other servers rather than replacing it. Then fully restart Claude Desktop (quit
and reopen). It reads MCP config only at launch.

`mw-mcp` must be on `PATH` (the standard MemoryWhale install); otherwise use its
absolute path as `command`. For a non-default database add
`"env": { "MEMORYWHALE_DATA_DIR": "/path/to/dir" }`. After restart, the tools
appear under the tools menu in the composer.

## Verify

```bash
command -v mw-mcp
```

On Windows PowerShell use `Get-Command mw-mcp`; in Command Prompt use
`where.exe mw-mcp`.

Quit and reopen Claude Desktop, then confirm `memorywhale` appears in the tools
menu with the six MemoryWhale tools. If the server is missing, the config file
was not found or was invalid JSON.

## Available capabilities

| Capability | Available |
| --- | --- |
| MCP memory access | Yes |
| Automatic execution capture | No |
| Memory-use guidance | No |

The six tools are `recent_errors`, `search_memory`, `get_context`, `remember`,
`similar_failures`, and `stats`. MCP access is not automatic capture. Commands
run outside MemoryWhale's terminal capture are not recorded.

Without the MCP server, the `mw` CLI still works: `mw context --last-error`,
`mw search "…"`, `mw remember "…"`.

## Example prompt

> Use MemoryWhale to check whether I encountered a similar failure before.

## Troubleshooting

- Run `command -v mw-mcp` in a terminal. If Claude Desktop was started from a
  GUI, it may not see a shell-only `PATH`; use the absolute path as `command`.
- Validate the JSON and restart Claude Desktop after every edit.
- If the wrong database opens, set `MEMORYWHALE_DATA_DIR` in the server `env`.
- Run `mw doctor` to check the MemoryWhale install.

## Uninstall

Delete the `memorywhale` entry from `mcpServers` in
`claude_desktop_config.json` and fully restart Claude Desktop. This does not
delete the MemoryWhale database.

# CrowClaw + MemoryWhale

[CrowClaw](https://github.com/subinium/CrowClaw) agents connect to MCP servers,
so they can use MemoryWhale's six local memory tools.

## Status

Verified against the custom-server shape shipped in
[`server.json`](server.json) and CrowClaw's dashboard MCP add flow as
documented in this repository on 2026-08-29. Custom (non-preset) servers take
`name`, `command`, `args`, and `custom: true`. Definitions persist under
`~/.crowclaw/`. Tool discovery is `GET /api/mcp/servers/memorywhale/tools`.

## Requirements

- MemoryWhale installed with `mw-mcp` on `PATH`.
- CrowClaw with the dashboard or access to its MCP REST API.

## Setup

CrowClaw supports custom (non-preset) MCP servers. Add MemoryWhale as one.

**From the dashboard:** open the CrowClaw dashboard → MCP → Add server,
and enter:

| Field | Value |
| --- | --- |
| Name | `memorywhale` |
| Command | `mw-mcp` |
| Args | *(none)* |

**Or via the REST API** the dashboard uses. Post the block from
[`server.json`](server.json) to `/api/mcp/servers`:

```json
{
  "name": "memorywhale",
  "command": "mw-mcp",
  "args": [],
  "custom": true
}
```

Either way the definition is persisted under `~/.crowclaw/`. `mw-mcp` must be on
`PATH` (the standard MemoryWhale install); otherwise use its absolute path as
`command`. For a non-default database add
`"env": { "MEMORYWHALE_DATA_DIR": "/path/to/dir" }`.

### Memory-use guidance

A preset's `systemPromptAppend` (or your agent's system prompt) teaches it
when to reach for the tools:

> When a build/test/deploy fails, before proposing a fix, use `search_memory`
> or `recent_errors` (MemoryWhale MCP) to check whether this failure has a known
> cause or a saved lesson. Once you've figured out why something failed or how a
> fix worked, use `remember` to save that conclusion.

## Verify

```bash
command -v mw-mcp
```

Then confirm tools were discovered:

```
GET /api/mcp/servers/memorywhale/tools
```

(or use the dashboard's per-server reconnect/tools view). You should see the
six tools: `recent_errors`, `search_memory`, `get_context`, `remember`,
`similar_failures`, and `stats`.

## Available capabilities

| Capability | Available |
| --- | --- |
| MCP memory access | Yes |
| Automatic execution capture | No |
| Memory-use guidance | Yes, via system prompt |

MCP access is not automatic execution capture. Without the MCP server, the
`mw` CLI still works: `mw context --last-error`, `mw search "…"`,
`mw remember "…"`.

## Example prompt

> Use MemoryWhale to check whether I encountered a similar failure before.

## Troubleshooting

- Run `command -v mw-mcp` from the environment CrowClaw's runtime uses.
- Reconnect the server in the dashboard if the add succeeded but tools are
  empty.
- If the wrong database opens, set `MEMORYWHALE_DATA_DIR` in `env`.
- Run `mw doctor` to check the MemoryWhale install.

## Uninstall

Remove the `memorywhale` server from the CrowClaw dashboard MCP page, or
delete the matching custom-server definition under `~/.crowclaw/`. Remove any
system-prompt instruction you added. This does not delete the MemoryWhale
database.

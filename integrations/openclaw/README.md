# OpenClaw + MemoryWhale

[OpenClaw](https://github.com/openclaw/openclaw) agents connect to MCP servers,
so they can use MemoryWhale's six local memory tools.

## Status

Verified against OpenClaw's
[MCP tools](https://github.com/openclaw/openclaw/blob/main/docs/tools/mcp.md)
documentation on 2026-08-29. Stdio servers are added with
`openclaw mcp add … --command`. Saving a definition is not enough;
`openclaw mcp doctor <name> --probe` checks reachability. Config lives under
`mcp.servers` in `~/.openclaw/openclaw.json` (JSON5).

## Requirements

- MemoryWhale installed with `mw-mcp` on `PATH`.
- OpenClaw CLI (and a running Gateway if you expect a live agent to pick up
  the server).

## Setup

Register the server with the CLI:

```bash
openclaw mcp add memorywhale --command mw-mcp
openclaw mcp doctor memorywhale --probe
```

The probe matters. Saving a definition proves nothing about reachability.

Or write it straight into config. Merge the block from
[`openclaw.mcp.json5`](openclaw.mcp.json5) into `~/.openclaw/openclaw.json`
(OpenClaw accepts JSON5):

```json5
{
  mcp: {
    servers: {
      memorywhale: { command: "mw-mcp", enabled: true },
    },
  },
}
```

`mw-mcp` must be on `PATH` (the standard MemoryWhale install); otherwise use its
absolute path as `command`. For a non-default database add
`env: { MEMORYWHALE_DATA_DIR: "/path/to/dir" }`. A running Gateway may need a
restart or runtime reload before it picks up a config-file change.

You can also add a stdio server from Control UI Settings → MCP → Add server.

### Memory-use guidance

OpenClaw agents read an `AGENTS.md` from their workspace (default
`~/.openclaw/workspace/AGENTS.md`). Add:

> When a build/test/deploy fails, before proposing a fix, use `search_memory`
> or `recent_errors` (MemoryWhale MCP) to check whether this failure has a known
> cause or a saved lesson. Once you've figured out why something failed or how a
> fix worked, use `remember` to save that conclusion.

## Verify

```bash
command -v mw-mcp
openclaw mcp doctor memorywhale --probe
```

The probe should show the six tools: `recent_errors`, `search_memory`,
`get_context`, `remember`, `similar_failures`, and `stats`.

## Available capabilities

| Capability | Available |
| --- | --- |
| MCP memory access | Yes |
| Automatic execution capture | No |
| Memory-use guidance | Yes, via workspace `AGENTS.md` |

MCP access is not automatic execution capture. Without the MCP server, the
`mw` CLI still works: `mw context --last-error`, `mw search "…"`,
`mw remember "…"`. With no integration at all, `mw ask` packages the last
failure onto your clipboard for any chat.

## Example prompt

> Use MemoryWhale to check whether I encountered a similar failure before.

## Troubleshooting

- Run `command -v mw-mcp` from the environment that launches OpenClaw.
- If `mcp add` succeeded but tools are missing, run
  `openclaw mcp doctor memorywhale --probe` and restart the Gateway.
- If the wrong database opens, set `MEMORYWHALE_DATA_DIR` in the server `env`.
- Run `mw doctor` to check the MemoryWhale install.

## Uninstall

Remove the server from Control UI Settings → MCP, or:

```bash
openclaw mcp unset memorywhale
```

Delete the `memorywhale` key from `mcp.servers` if you added it by hand.
Remove any `AGENTS.md` instruction you added. This does not delete the
MemoryWhale database.

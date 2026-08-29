# Continue + MemoryWhale

[Continue](https://github.com/continuedev/continue) connects to MCP servers in
agent mode, so it can use MemoryWhale's six local memory tools.

## Status

Verified against Continue's
[MCP deep dive](https://docs.continue.dev/customize/deep-dives/mcp) and
[config.yaml `mcpServers` reference](https://docs.continue.dev/reference/config)
on 2026-08-29. The documented project layout is a YAML block under
`.continue/mcpServers/`. A `mcpServers` list in `config.yaml` is still in the
YAML reference.

## Requirements

- MemoryWhale installed with `mw-mcp` on `PATH`.
- Continue in agent mode (MCP tools are not used outside agent mode).

## Setup

### Project MCP block (documented layout)

Create `.continue/mcpServers/memorywhale.yaml` in the workspace:

```yaml
name: MemoryWhale
version: 0.0.1
schema: v1
mcpServers:
  - name: memorywhale
    command: mw-mcp
    args: []
```

### Global config.yaml

You can instead add the `mcpServers` list from [`config.yaml`](config.yaml)
to `~/.continue/config.yaml`:

```yaml
mcpServers:
  - name: memorywhale
    command: mw-mcp
    args: []
```

`mcpServers` entries are list items. The leading `-` matters, and mixing
tabs with spaces silently breaks YAML parsing. If you already have an
`mcpServers:` list, add `memorywhale` as another item rather than replacing it.

`mw-mcp` must be on `PATH` (the standard MemoryWhale install); otherwise use its
absolute path as `command`. For a non-default database add an `env` map:
`env: { MEMORYWHALE_DATA_DIR: "/path/to/dir" }`.

Continue does not always pick up MCP edits live. Run Reload Window (VS Code)
or reload the extension so it re-reads the config.

### Memory-use guidance

A Continue rule / system message teaches the agent when to use the tools:

> When a build/test/deploy fails, before proposing a fix, use `search_memory`
> or `recent_errors` (MemoryWhale MCP) to check whether this failure has a known
> cause or a saved lesson. Once you've figured out why something failed or how a
> fix worked, use `remember` to save that conclusion.

## Verify

```bash
command -v mw-mcp
```

Switch Continue to agent mode and confirm the six tools:
`recent_errors`, `search_memory`, `get_context`, `remember`,
`similar_failures`, and `stats`.

## Available capabilities

| Capability | Available |
| --- | --- |
| MCP memory access | Yes |
| Automatic execution capture | No |
| Memory-use guidance | Yes, optional rule |

MCP access is not automatic execution capture. Without the MCP server, the
`mw` CLI still works: `mw context --last-error`, `mw search "…"`,
`mw remember "…"`.

## Example prompt

> Use MemoryWhale to check whether I encountered a similar failure before.

## Troubleshooting

- Confirm Continue is in agent mode.
- Validate YAML (list dashes, no tabs).
- Reload the window after editing config.
- Run `command -v mw-mcp` from the environment Continue uses.
- If the wrong database opens, set `MEMORYWHALE_DATA_DIR` in `env`.
- Run `mw doctor` to check the MemoryWhale install.

## Uninstall

Delete `.continue/mcpServers/memorywhale.yaml` and/or the `memorywhale` item
from `mcpServers` in `~/.continue/config.yaml`. Remove any rule you added.
Reload Continue. This does not delete the MemoryWhale database.

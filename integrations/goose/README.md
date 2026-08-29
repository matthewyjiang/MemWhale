# Goose + MemoryWhale

[Goose](https://github.com/block/goose) calls MCP servers "extensions", so it
can use MemoryWhale's six local memory tools.

## Status

Verified against Goose's
[Using extensions](https://goose-docs.ai/docs/getting-started/using-extensions)
and [configuration files](https://goose-docs.ai/docs/guides/config-files)
documentation on 2026-08-29. Desktop setup is Add custom extension (Standard
IO). File setup is `~/.config/goose/config.yaml` under `extensions:`, with
`type: stdio` and `cmd`.

## Requirements

- MemoryWhale installed with `mw-mcp` on `PATH`.
- Goose CLI or desktop.

## Setup

### Wizard (recommended)

```bash
goose configure
```

Choose Add Extension → Command-line Extension, then:

| Prompt | Value |
| --- | --- |
| Name | `memorywhale` |
| Command | `mw-mcp` |
| Timeout | `300` (default) |

In the desktop app the same lives under Settings → Extensions → Add.

### Config file

Add this under `extensions:` in `~/.config/goose/config.yaml`
([`config.yaml`](config.yaml)):

```yaml
extensions:
  memorywhale:
    type: stdio
    name: memorywhale
    cmd: mw-mcp
    args: []
    enabled: true
    timeout: 300
    env_keys: []
    envs: {}
    description: "MemoryWhale persistent local memory"
```

`mw-mcp` must be on `PATH` (the standard MemoryWhale install); otherwise use its
absolute path as `cmd`. For a non-default database, add
`MEMORYWHALE_DATA_DIR` to `envs` (e.g. `envs: { MEMORYWHALE_DATA_DIR: "/path/to/dir" }`).

### Memory-use guidance

A hint in `.goosehints` (or the session system prompt) teaches Goose when to
use the tools:

> When a build/test/deploy fails, before proposing a fix, use `search_memory`
> or `recent_errors` (MemoryWhale MCP) to check whether this failure has a known
> cause or a saved lesson. Once you've figured out why something failed or how a
> fix worked, use `remember` to save that conclusion.

## Verify

```bash
command -v mw-mcp
```

Start a Goose session and confirm the `memorywhale` extension is enabled with
the six tools: `recent_errors`, `search_memory`, `get_context`, `remember`,
`similar_failures`, and `stats`.

## Available capabilities

| Capability | Available |
| --- | --- |
| MCP memory access | Yes |
| Automatic execution capture | No |
| Memory-use guidance | Yes, optional `.goosehints` |

MCP access is not automatic execution capture. Without the MCP server, the
`mw` CLI still works: `mw context --last-error`, `mw search "…"`,
`mw remember "…"`.

## Example prompt

> Use MemoryWhale to check whether I encountered a similar failure before.

## Troubleshooting

- Run `command -v mw-mcp` from the environment that launches Goose.
- Confirm `type: stdio` and `cmd: mw-mcp` (Goose uses `cmd`, not `command`).
- Restart Goose after editing `config.yaml`.
- If the wrong database opens, set `MEMORYWHALE_DATA_DIR` in `envs`.
- Run `mw doctor` to check the MemoryWhale install.

## Uninstall

Disable or delete the `memorywhale` extension in `goose configure` / the
desktop Extensions UI, or remove the `memorywhale:` block from
`~/.config/goose/config.yaml`. Remove any `.goosehints` line you added. This
does not delete the MemoryWhale database.

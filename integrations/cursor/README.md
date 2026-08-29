# Cursor + MemoryWhale

Give Cursor's agent the same local MemoryWhale tools: it can read what already
failed and write down what it figured out. Two pieces, an MCP server and a Rule.

## Status

Verified against Cursor's [MCP](https://cursor.com/docs/context/mcp) and
[CLI MCP](https://cursor.com/docs/cli/mcp) documentation on 2026-08-29.
Project config is `.cursor/mcp.json`. User config is `~/.cursor/mcp.json`.
Both files are merged; a duplicate name in the project file wins.

## Requirements

- MemoryWhale installed with `mw-mcp` on `PATH`.
- Cursor (editor or CLI `agent`).
- Optional: the repository Rule file [`memorywhale.mdc`](memorywhale.mdc).

## Setup

### Connect the MCP server

`mw-mcp` is a local stdio MCP server. Point Cursor at it via `mcp.json`:

- This project only: copy [`mcp.json`](mcp.json) to `.cursor/mcp.json` in
  your project root.
- Every project: put the same JSON in `~/.cursor/mcp.json`.

```json
{
  "mcpServers": {
    "memorywhale": {
      "command": "mw-mcp"
    }
  }
}
```

`mw-mcp` must be on `PATH` (the standard MemoryWhale install). If it isn't, use
its absolute path as `command`. To point at a non-default database, add
`"env": { "MEMORYWHALE_DATA_DIR": "/path/to/dir" }`.

### Add the Rule

The MCP server gives Cursor the tools; this Rule teaches it when to reach
for them (recurring failures, "how did we fix this last time", and saving a
conclusion once a fix is found).

- This project only: copy [`memorywhale.mdc`](memorywhale.mdc) to
  `.cursor/rules/memorywhale.mdc`.
- Every project: add it via Cursor Settings → Rules → User Rules.

It is an "Agent Requested" rule (`alwaysApply: false`), so Cursor pulls it in
only when the description matches what you're doing.

## Verify

```bash
command -v mw-mcp
```

In the editor, open Settings → MCP and confirm `memorywhale` is listed. From
the Cursor CLI, which uses the same config:

```bash
agent mcp list
agent mcp list-tools memorywhale
```

You should see the six tools: `recent_errors`, `search_memory`, `get_context`,
`remember`, `similar_failures`, and `stats`.

## Available capabilities

| Capability | Available |
| --- | --- |
| MCP memory access | Yes |
| Automatic execution capture | No |
| Memory-use guidance | Yes, optional Rule |

MCP access is not automatic execution capture. Cursor-run commands are recorded
only through MemoryWhale's normal terminal capture, `mw-run`, or `mw-remember`.

The Rule falls back to the `mw` CLI, so even with only the binaries installed,
Cursor can run `mw context --last-error` / `mw search "…"` / `mw remember "…"`.
With no editor integration at all, `mw ask` packages the last failure onto
your clipboard for any chat.

## Example prompt

> Use MemoryWhale to check whether I encountered a similar failure before.

## Troubleshooting

- Run `command -v mw-mcp` in the environment that launches Cursor. Use an
  absolute `command` if the GUI `PATH` differs from your shell.
- Confirm you edited `.cursor/mcp.json` or `~/.cursor/mcp.json`, not some other
  client's file.
- Restart Cursor or reload MCP if the server does not appear.
- If the wrong database opens, set `MEMORYWHALE_DATA_DIR` in the server `env`.
- Run `mw doctor` to check the MemoryWhale install.

## Uninstall

Delete the `memorywhale` entry from `.cursor/mcp.json` and/or
`~/.cursor/mcp.json`. Remove `.cursor/rules/memorywhale.mdc` or the matching
user Rule. Restart Cursor. This does not delete the MemoryWhale database.

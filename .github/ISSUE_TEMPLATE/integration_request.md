---
name: Integration request
about: Add an MCP client / editor to integrations/
title: "Add a <tool> integration"
labels: documentation, good first issue
---

## Summary
<!-- Which tool, and which MemoryWhale seam it uses (stdio MCP, CLI, gateway). -->
`<tool>` should get an `integrations/<tool>/` guide. If it can run a local
stdio MCP server, it can use MemoryWhale's six tools (`recent_errors`,
`search_memory`, `get_context`, `remember`, `similar_failures`, `stats`).
If it does not speak MCP, say which seam it uses instead.

## Why it's a good first issue
Docs + config only, mirroring an existing folder (e.g. `integrations/codex/`).
No Rust.

## Canonical guide structure

Follow [integrations/TEMPLATE.md](../../integrations/TEMPLATE.md). Every
README must use these headings, in this order:

1. Status
2. Requirements
3. Setup
4. Verify
5. Available capabilities
6. Example prompt
7. Troubleshooting
8. Uninstall

Write "Not applicable" plus a reason when a section does not apply. Do not
copy another client’s commands or paths; verify them against the tool’s
current documentation.

## What to do
1. Create `integrations/<tool>/`. If the client speaks stdio MCP, include a
   config snippet whose command is `mw-mcp`. If it does not, document the
   actual seam (CLI plugin, hosted gateway, PR workflow, or similar) and skip
   an `mw-mcp` registration.
2. **Verify the exact config format/location against the tool's current docs** —
   don't assume; formats change between versions.
3. Add a `README.md` using `integrations/TEMPLATE.md`. For MCP clients, declare
   verified capabilities, register the server, include the `mw-mcp`-on-PATH
   note, document `MEMORYWHALE_DATA_DIR` for a non-default DB, and distinguish
   MCP access from automatic execution capture. For other clients, write
   "Not applicable" plus a reason on sections that do not apply.
4. List the tool in `integrations/README.md`.

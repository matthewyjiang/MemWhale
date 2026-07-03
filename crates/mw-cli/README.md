# memorywhale-cli

Local-first terminal memory. Records commands, arguments, output, errors, and
whole sessions into local SQLite, so what already failed stays searchable —
across crashes, SSH drops, and machine switches. Nothing is uploaded.

Installs these binaries: `mw`, `mw-run`, `mw-remember`, `mw-serve`, `mw-view`,
`mw-recover`, `mw-screenshot`, and `mw-mcp` (a Model Context Protocol server so
an AI agent can query your memory directly).

```bash
cargo install memorywhale-cli
mw                       # record a shell session; it explains itself on first run
mw-run -- cargo build    # capture one command's output + exit code
mw search "linker error" # full-text search your history
mw context --last-error  # a paste-ready digest for an AI agent
mw-serve                 # local web dashboard (works headless, e.g. on a Jetson)
```

Full documentation, the web dashboard, team sharing, and AI-agent integration:
<https://github.com/wuisabel-gif/MemWhale>.

Captured output is scrubbed for common secret shapes before storage; set
`MEMORYWHALE_NO_REDACT=1` to store raw. License: MIT.

# CLI reference

All commands ship as prebuilt binaries (see the README's Install section). If
you're working from a source checkout instead, prefix any command with
`cargo run -p memorywhale-cli --bin <name> --` from the repo root, e.g.
`cargo run -p memorywhale-cli --bin mw -- --notes "…"`.

## mw — record sessions

```bash
mw --notes "Jetson build debugging"   # record a whole shell session until exit
mw --live --notes "project:demo"      # autosave to SQLite every few seconds
mw list                               # list recorded sessions
mw show 1                             # print the full transcript of a session
mw search "linker error"              # search commands, output, notes, transcripts
mw mark "before the risky flash"      # bookmark the current debugging moment
mw replay 12                          # rerun a saved command run
mw demo                               # seed a small demo dataset to explore
mw rm 5                               # delete a session (+ its transcript); mw rm command <id> for a run
mw discard                            # inside a recording: throw the current session away
mw context [project:name] [--last-error] [--limit N]   # digest for AI agents
mw doctor                             # check the install
mw export [project:name]              # export a bundle (Markdown + JSON + SQLite)
mw import <bundle|sqlite>             # merge another machine's export
mw push <ssh-host>                    # send this machine's memory to another (scp + remote import)
mw pull <ssh-host> [path]             # the reverse: copy another machine's memory here and merge it
mw global on|off|status               # auto-record every new terminal
```

`mw` starts a recorded subshell; run commands normally, then `exit` or Ctrl-D
to stop. The raw transcript lands in the data folder, searchable metadata in
SQLite. `--live` matters for SSH sessions and sudden shutdown risk: if the
terminal dies before `exit`, the last autosaved transcript is still there.
Recorded a garbage terminal? Type `mw discard` inside it before exiting, or
`mw rm <id>` after the fact.

## mw-run — capture one command

```bash
mw-run --notes "Check the Rust backend" -- cargo check
```

Output still streams to your terminal while a copy (stdout, stderr, exit code,
cwd, argv) is saved. `mw-run` exits with the same exit code as the command.

## mw-remember — save output you already have

```bash
mw-remember \
  --cwd . \
  --exit-code 127 \
  --stderr "zsh:1: command not found: cargo" \
  --notes "Rust verification failed because cargo was missing" \
  -- cargo check --manifest-path src-tauri/Cargo.toml
```

## mw-screenshot — opt-in visual evidence

```bash
mw-screenshot --notes "VS Code showed the TypeScript warning"
```

Local-only and opt-in. On headless machines (e.g. a Jetson without a display)
screenshot capture may fail; terminal memory recording still works.

## mw-serve / mw-view / mw-recover / mw-mcp

```bash
mw-serve [--host addr] [--port n] [--token secret]  # web dashboard
mw-view <id>                                        # open one memory directly
mw-recover                                          # recover interrupted recordings
mw-mcp                                              # MCP server for AI agents (stdio)
```

## Data location

By default the SQLite database lives in the local app data directory. Set
`MEMORYWHALE_DATA_DIR` for an explicit location:

```bash
MEMORYWHALE_DATA_DIR=/tmp/memorywhale-data mw-run -- echo "saved here"
```

## Secret redaction

Captured stdout/stderr/notes/transcripts are scrubbed for common secret shapes
(API keys, tokens, `password=`, PEM blocks) before they reach SQLite. Set
`MEMORYWHALE_NO_REDACT=1` to store raw text.

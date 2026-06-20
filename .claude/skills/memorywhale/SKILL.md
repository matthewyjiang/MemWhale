---
name: memorywhale
description: Record terminal work (commands, errors, fixes, sessions) into MemoryWhale's local SQLite memory, and recall prior attempts before debugging. Use when running shell commands that may fail, debugging build/environment errors, or continuing work across sessions or machines (e.g. a Jetson and a laptop) where past terminal context matters.
---

# MemoryWhale: durable terminal memory for agents

MemoryWhale stores commands, arguments, exit codes, stdout/stderr, notes, and
whole recorded sessions in a **local** SQLite database. Use it to (1) **recall**
what was already tried before attempting something, and (2) **record** what you
do so a future session — or a different machine, or a different agent — inherits
the reasoning trail instead of starting cold.

Everything is local; nothing is uploaded. The database is at
`<data_local>/MemoryWhale/memorywhale.sqlite3`
(`~/.local/share/...` on Linux, `~/Library/Application Support/...` on macOS).

## When to use this skill

- Before debugging a build/environment error — check if it was solved before.
- After a command fails, and again after you find the fix.
- When work spans multiple terminals, sessions, or machines.

## Recall first (before you act)

Search past memory for the command or error you're about to deal with. Use the
bundled helper:

```bash
bash "$CLAUDE_SKILL_DIR/recall.sh" "linker cc not found"
```

It prints prior command runs and sessions that match, including what was run
**right after** a past failure (often the fix). If a known-good fix exists,
prefer it over re-deriving from scratch.

Equivalent direct query:

```bash
sqlite3 ~/.local/share/MemoryWhale/memorywhale.sqlite3 \
  "SELECT id, command, exit_code, notes FROM command_runs
   WHERE command LIKE '%cargo%' OR stderr LIKE '%linker%' ORDER BY id DESC LIMIT 10;"
```

## Record as you go

Log a notable command — especially a failure, and the command that fixed it:

```bash
mw-remember --cwd "$(pwd)" --exit-code "$?" \
  --stderr "<the error output>" \
  --notes "project:<name> what this was / why it matters" -- <command> [args]
```

For an exploratory stretch, record the whole session (every command + output):

```bash
mw --notes "project:<name> what you're debugging"
#   ...work...   then:  exit   (wait for: mw: recorded session #N)
```

Tag related work across terminals with the same `project:<name>` so it groups
automatically.

## Inspect / browse

- `mw-view <id>` — open one memory as a local web page (with suggested next steps).
- `mw-serve` — serve the whole memory as a dashboard (`http://localhost:7071/`,
  or over the LAN for headless machines). Includes a `/graph` and project views.
- `mw-recover` — import any interrupted session transcript that didn't save.

## Setup (if the binaries aren't installed)

```bash
cd src-tauri
cargo build --release --bin mw --bin mw-remember --bin mw-serve --bin mw-view --bin mw-recover
mkdir -p ~/.local/bin && cp target/release/{mw,mw-remember,mw-serve,mw-view,mw-recover} ~/.local/bin/
# macOS only: re-sign copied binaries or they get "Killed: 9"
command -v codesign >/dev/null && codesign --force --sign - ~/.local/bin/mw* || true
```

See `SOP.md` and `DEBUG.md` in the repo for the full procedure.

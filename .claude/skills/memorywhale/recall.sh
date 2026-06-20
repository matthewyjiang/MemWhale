#!/usr/bin/env bash
# recall.sh "<error or command text>"
# Search MemoryWhale's local memory for prior runs/sessions matching the text,
# and show what was run right after a past failure (often the fix).
set -euo pipefail

q="${1:-}"
if [ -z "$q" ]; then
  echo "usage: recall.sh \"<error or command text>\"" >&2
  exit 1
fi

db="${MEMORYWHALE_DB:-$HOME/.local/share/MemoryWhale/memorywhale.sqlite3}"
[ -f "$db" ] || db="$HOME/Library/Application Support/MemoryWhale/memorywhale.sqlite3"
if [ ! -f "$db" ]; then
  echo "MemoryWhale database not found (looked in ~/.local/share and ~/Library/Application Support)."
  exit 0
fi

# Ensure both tables exist so queries don't fail on a partially-used database.
sqlite3 "$db" "
  CREATE TABLE IF NOT EXISTS command_runs (id INTEGER PRIMARY KEY, command TEXT NOT NULL,
    argv_json TEXT NOT NULL DEFAULT '', cwd TEXT, exit_code INTEGER, stdout TEXT NOT NULL DEFAULT '',
    stderr TEXT NOT NULL DEFAULT '', notes TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL DEFAULT '');
  CREATE TABLE IF NOT EXISTS sessions (id INTEGER PRIMARY KEY, shell TEXT, cwd TEXT,
    transcript_path TEXT NOT NULL DEFAULT '', transcript TEXT NOT NULL DEFAULT '',
    notes TEXT NOT NULL DEFAULT '', started_at TEXT NOT NULL DEFAULT '',
    ended_at TEXT NOT NULL DEFAULT '', byte_count INTEGER NOT NULL DEFAULT 0);" 2>/dev/null || true

# Escape single quotes for SQL.
esc=$(printf '%s' "$q" | sed "s/'/''/g")

echo "## Past command runs matching: $q"
sqlite3 -box "$db" "
  SELECT id, command, exit_code AS exit, substr(notes,1,60) AS notes
  FROM command_runs
  WHERE command LIKE '%$esc%' OR stderr LIKE '%$esc%' OR notes LIKE '%$esc%'
  ORDER BY id DESC LIMIT 10;"

echo
echo "## What was run right after a matching failure (likely fixes)"
sqlite3 -box "$db" "
  SELECT nxt.command AS fix_command, nxt.notes AS notes
  FROM command_runs f
  JOIN command_runs nxt ON nxt.created_at > f.created_at
  WHERE (f.command LIKE '%$esc%' OR f.stderr LIKE '%$esc%') AND f.exit_code <> 0
    AND nxt.id = (SELECT id FROM command_runs WHERE created_at > f.created_at ORDER BY created_at ASC LIMIT 1)
  LIMIT 5;"

echo
echo "## Sessions mentioning it"
sqlite3 -box "$db" "
  SELECT id, started_at, substr(notes,1,50) AS notes
  FROM sessions
  WHERE transcript LIKE '%$esc%' OR notes LIKE '%$esc%'
  ORDER BY id DESC LIMIT 5;"

---
name: memorywhale
description: Query durable terminal-debugging memory recorded by MemoryWhale. Use when debugging a failure that may have happened before, when you need the exact error/flags/output from an earlier attempt, or when the user asks "how did we fix this last time?". Works across machines and past sessions.
---

# MemoryWhale memory

MemoryWhale records terminal commands, their arguments, exit codes, output, and
errors into a local SQLite database. That history survives crashes, SSH drops,
and switching machines — it's the record of what was already tried.

## When to use this

- A build/test/deploy is failing and it might have failed before.
- You need the *exact* earlier error text, flags, or working directory, not a
  paraphrase.
- The user references past work ("last week", "on the Jetson", "how did I fix").

## How to pull the memory

Prefer the MCP tools if the `memorywhale` MCP server is connected (tools:
`recent_errors`, `search_memory`, `get_context`). Otherwise shell out:

```bash
mw context                 # recent failed commands + sessions, compact
mw context --last-error    # just the most recent failure, with its error tail
mw context project:NAME    # scope to a project tag
```

Search for a specific term across commands, output, and notes:

```bash
mw-run --help              # (reference) how new runs get recorded
mw list                    # recorded sessions; `mw show <id>` prints a transcript
```

## Reading the output

`mw context` returns failed commands with `cwd`, exit code, and the tail of
their error, plus recent sessions you can expand with `mw show <id>`. Use it to
avoid re-deriving context: check whether the current failure already has a known
cause before proposing a fix.

## Note

Captured output is secret-redacted on the way in, but treat it as real project
data. Everything is local — nothing is uploaded.

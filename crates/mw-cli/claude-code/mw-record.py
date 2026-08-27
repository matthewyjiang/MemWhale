#!/usr/bin/env python3
"""Claude Code hook: record every Bash command the agent runs into MemoryWhale.

Handles both PostToolUse (successful Bash calls) and PostToolUseFailure (failed
Bash calls). Reads the hook JSON payload from stdin, and — only for the Bash
tool — shells out to `mw-remember` with the command, its output, and its exit
status. Field names are read defensively with fallbacks, since hook payload
shape can vary slightly across Claude Code versions; if a field is missing
this still records what it can.

Never fails the tool call: any error here is swallowed and the hook exits 0,
so a MemoryWhale hiccup can't block your agent session.

Install: `mw integrate claude` (see integrations/claude-code/README.md).
"""
import json
import re
import subprocess
import sys
import shutil

MAX_OUTPUT = 20_000  # cap what we pass as args; mw-remember still redacts secrets
EXIT_CODE_RE = re.compile(r"(?:exit(?:\s+|-)?code\s*[:=]?\s*|exited with code\s+)(\d+)", re.I)


def first(d, *keys, default=""):
    for k in keys:
        v = d.get(k)
        if v:
            return v
    return default


def as_dict(value):
    return value if isinstance(value, dict) else {}


def bash_exit_code(payload, tool_response=None):
    """Return a Bash process exit code only when the payload provides one."""
    for source in (as_dict(payload), as_dict(tool_response)):
        for key in ("exit_code", "exitCode", "return_code", "returnCode"):
            value = source.get(key)
            if value is None or value == "":
                continue
            try:
                code = int(value)
            except (TypeError, ValueError):
                continue
            if 0 <= code <= 255:
                return str(code)

    error_text = str(payload.get("error", ""))
    match = EXIT_CODE_RE.search(error_text)
    if match:
        try:
            code = int(match.group(1))
        except ValueError:
            return None
        if 0 <= code <= 255:
            return str(code)
    return None


def main():
    try:
        payload = json.load(sys.stdin)
    except Exception:
        return  # not JSON, or nothing to read — nothing to record

    if not isinstance(payload, dict):
        return

    if payload.get("tool_name") != "Bash":
        return

    tool_input = as_dict(payload.get("tool_input"))
    command = str(tool_input.get("command") or "").strip()
    if not command:
        return

    cwd = payload.get("cwd") or tool_input.get("cwd") or ""
    event = payload.get("hook_event_name", "")
    exit_code = None
    if event == "PostToolUseFailure":
        stdout = ""
        stderr = str(payload.get("error", ""))[:MAX_OUTPUT]
        exit_code = bash_exit_code(payload)
    else:
        tool_response = as_dict(payload.get("tool_response"))
        stdout = str(first(tool_response, "stdout", "output"))[:MAX_OUTPUT]
        stderr = str(first(tool_response, "stderr"))[:MAX_OUTPUT]
        is_error = bool(
            tool_response.get("is_error")
            or tool_response.get("isError")
            or tool_response.get("interrupted")
        )
        exit_code = bash_exit_code(payload, tool_response) or ("1" if is_error else "0")

    mw_remember = shutil.which("mw-remember")
    if not mw_remember:
        return  # MemoryWhale not installed/on PATH — silently skip

    remember_args = [
        mw_remember,
        "--cwd", cwd,
        "--stdout", stdout,
        "--stderr", stderr,
        "--notes", "agent:claude-code",
    ]
    if exit_code is not None:
        remember_args.extend(["--exit-code", exit_code])
    remember_args.extend(["--", command])

    try:
        subprocess.run(
            remember_args,
            capture_output=True,
            timeout=10,
        )
    except Exception:
        pass  # never let a recording failure interrupt the agent


def _selftest():
    """`python3 mw-record.py --selftest` — sanity-checks the payload parsing
    without touching mw-remember or subprocess."""
    assert first({"a": "x"}, "a", "b") == "x"
    assert first({"b": "y"}, "a", "b") == "y"
    assert first({}, "a", "b", default="z") == "z"
    assert first({"a": ""}, "a", "b") == "" or first({"a": "", "b": "y"}, "a", "b") == "y"

    long_text = "x" * 100
    assert len(long_text[:MAX_OUTPUT]) <= MAX_OUTPUT

    failure_payload = {
        "hook_event_name": "PostToolUseFailure",
        "tool_name": "Bash",
        "tool_input": {"command": "false"},
        "error": "Exit code 1",
    }
    assert bash_exit_code(failure_payload) == "1"
    assert bash_exit_code({"error": "permission denied before launch"}) is None

    print("mw-record.py: selftest OK")


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--selftest":
        _selftest()
    else:
        try:
            main()
        except Exception:
            pass  # never let a recording failure interrupt the agent

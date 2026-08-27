#!/usr/bin/env python3
"""Rho hook: record bash/powershell tool calls into MemoryWhale.

Handles the observational `after_tool_use` event. Rho sends one JSON envelope
on stdin. Command text and output are included only when the payload carries
them (today `after_tool_use` is status/failure/duration; `shell_command` is
read defensively in case a later schema adds it). Failed calls without a
command are still recorded under the tool name so the error is not dropped.

Never fails the tool call: observational hooks are ignored by Rho on error,
and this script also swallows its own failures so a MemoryWhale hiccup cannot
affect the session.

Install: `mw integrate rho` (see integrations/rho/README.md).
"""
import json
import subprocess
import sys
import shutil

MAX_OUTPUT = 20_000  # cap what we pass as args; mw-remember still redacts secrets


def first(d, *keys, default=""):
    for k in keys:
        v = d.get(k)
        if v:
            return v
    return default


def as_dict(value):
    return value if isinstance(value, dict) else {}


def command_from(body):
    cap = as_dict(body.get("capability"))
    shell = str(cap.get("shell_command") or "").strip()
    if shell:
        return shell
    parts = []
    exe = cap.get("executable")
    if exe:
        parts.append(str(exe))
    args = cap.get("arguments") or []
    if isinstance(args, list):
        parts.extend(str(a) for a in args)
    return " ".join(parts).strip()


def cwd_from(payload, body):
    cap = as_dict(body.get("capability"))
    workspace = as_dict(payload.get("workspace"))
    return str(
        first(cap, "working_directory")
        or first(workspace, "root")
        or ""
    )


def main():
    try:
        payload = json.load(sys.stdin)
    except Exception:
        return  # not JSON, or nothing to read — nothing to record

    if not isinstance(payload, dict):
        return

    event = payload.get("event")
    if event and event != "after_tool_use":
        return

    body = payload.get("payload") if isinstance(payload.get("payload"), dict) else payload
    tool = as_dict(body.get("tool"))
    tool_name = str(tool.get("name") or "")
    if tool_name not in ("bash", "powershell"):
        return

    status = str(body.get("status") or "")
    failed = bool(status) and status != "succeeded"
    command = command_from(body)
    if not command:
        if not failed:
            return
        command = tool_name

    cwd = cwd_from(payload, body)
    failure = as_dict(body.get("failure"))
    kind = str(failure.get("kind") or "").strip()
    message = str(failure.get("message") or "").strip()
    if kind and message:
        stderr = f"{kind}: {message}"[:MAX_OUTPUT]
    elif kind:
        stderr = kind[:MAX_OUTPUT]
    else:
        stderr = message[:MAX_OUTPUT]
    stdout = ""
    exit_code = None if not status else ("1" if failed else "0")

    mw_remember = shutil.which("mw-remember")
    if not mw_remember:
        return  # MemoryWhale not installed/on PATH — silently skip

    remember_args = [
        mw_remember,
        "--cwd", cwd,
        "--stdout", stdout,
        "--stderr", stderr,
        "--notes", "agent:rho",
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
    """`python3 mw-record.py --selftest` — sanity-checks payload parsing
    without touching mw-remember or subprocess."""
    assert first({"a": "x"}, "a", "b") == "x"
    assert first({"b": "y"}, "a", "b") == "y"
    assert first({}, "a", "b", default="z") == "z"

    envelope = {
        "schema_version": 2,
        "event": "after_tool_use",
        "workspace": {"root": "/work"},
        "payload": {
            "tool": {"name": "bash", "call_id": "call-1"},
            "status": "failed",
            "failure": {"kind": "tool", "message": "exit 1"},
            "duration_ms": 12,
        },
    }
    body = envelope["payload"]
    assert command_from(body) == ""
    assert cwd_from(envelope, body) == "/work"
    assert as_dict(body.get("failure")).get("message") == "exit 1"

    with_command = {
        "capability": {
            "operation": "execute_process",
            "working_directory": "/tmp",
            "executable": "bash",
            "arguments": ["-lc"],
            "shell_command": "cargo test",
        }
    }
    assert command_from(with_command) == "cargo test"
    assert cwd_from({}, with_command) == "/tmp"

    print("mw-record.py: selftest OK")


if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "--selftest":
        _selftest()
    else:
        main()

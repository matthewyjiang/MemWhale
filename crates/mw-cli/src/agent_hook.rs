//! Parse Claude Code and Rho hook JSON into a [`CommandRecord`].
//!
//! The installer names the client on the argv (`--from-hook claude` /
//! `--from-hook rho`). This module does not guess the client from JSON.

use std::sync::OnceLock;

use regex::Regex;
use serde_json::Value;

use crate::remember::CommandRecord;

const MAX_OUTPUT: usize = 20_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Agent {
    Claude,
    Rho,
}

impl Agent {
    pub fn parse(name: &str) -> Option<Self> {
        match name {
            "claude" | "claude-code" => Some(Self::Claude),
            "rho" => Some(Self::Rho),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Rho => "rho",
        }
    }
}

pub fn record_from_slice(bytes: &[u8], agent: Agent) -> Option<CommandRecord> {
    let value: Value = serde_json::from_slice(bytes).ok()?;
    record_from_value(&value, agent)
}

pub fn record_from_value(payload: &Value, agent: Agent) -> Option<CommandRecord> {
    match agent {
        Agent::Claude => payload.as_object().and_then(claude),
        Agent::Rho => rho(payload),
    }
}

fn claude(payload: &serde_json::Map<String, Value>) -> Option<CommandRecord> {
    if payload.get("tool_name").and_then(Value::as_str) != Some("Bash") {
        return None;
    }
    let tool_input = as_object(payload.get("tool_input"));
    let command = tool_input
        .and_then(|input| input.get("command"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    if command.is_empty() {
        return None;
    }

    let cwd = nonempty_str(payload.get("cwd"))
        .map(str::to_string)
        .or_else(|| {
            tool_input.and_then(|input| nonempty_str(input.get("cwd")).map(str::to_string))
        });
    let event = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or("");

    let (stdout, stderr, exit_code) = if event == "PostToolUseFailure" {
        let stderr = truncate(
            payload
                .get("error")
                .map(value_as_string)
                .unwrap_or_default(),
        );
        let exit_code = bash_exit_code(payload, None);
        (String::new(), stderr, exit_code)
    } else {
        let tool_response = as_object(payload.get("tool_response"));
        let stdout = truncate(first_str(tool_response, &["stdout", "output"]));
        let stderr = truncate(first_str(tool_response, &["stderr"]));
        let is_error = tool_response.is_some_and(|response| {
            boolish(response.get("is_error"))
                || boolish(response.get("isError"))
                || boolish(response.get("interrupted"))
        });
        let exit_code =
            bash_exit_code(payload, tool_response).or(Some(if is_error { 1 } else { 0 }));
        (stdout, stderr, exit_code)
    };

    Some(CommandRecord {
        cwd,
        exit_code,
        stdout,
        stderr,
        notes: "agent:claude-code".to_string(),
        command_parts: vec![command],
        capture_kind: "full".to_string(),
    })
}

fn rho(payload: &Value) -> Option<CommandRecord> {
    let event = payload.get("event").and_then(Value::as_str);
    if event.is_some_and(|event| event != "after_tool_use") {
        return None;
    }
    let body = payload
        .get("payload")
        .and_then(Value::as_object)
        .or_else(|| payload.as_object())?;
    let tool = as_object(body.get("tool"));
    let tool_name = tool
        .and_then(|tool| tool.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("");
    if tool_name != "bash" && tool_name != "powershell" {
        return None;
    }

    let status = body.get("status").and_then(Value::as_str).unwrap_or("");
    let failed = !status.is_empty() && status != "succeeded";
    let mut command = command_from(body);
    if command.is_empty() {
        if !failed {
            return None;
        }
        command = tool_name.to_string();
    }

    let cwd = cwd_from(payload, body);
    let failure = as_object(body.get("failure"));
    let kind = failure
        .and_then(|failure| failure.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let message = failure
        .and_then(|failure| failure.get("message"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let stderr = truncate(if !kind.is_empty() && !message.is_empty() {
        format!("{kind}: {message}")
    } else if !kind.is_empty() {
        kind.to_string()
    } else {
        message.to_string()
    });
    let exit_code = if status.is_empty() {
        None
    } else if failed {
        Some(1)
    } else {
        Some(0)
    };

    Some(CommandRecord {
        cwd,
        exit_code,
        stdout: String::new(),
        stderr,
        notes: "agent:rho".to_string(),
        command_parts: vec![command],
        capture_kind: "full".to_string(),
    })
}

fn command_from(body: &serde_json::Map<String, Value>) -> String {
    let cap = as_object(body.get("capability"));
    let shell = cap
        .and_then(|cap| cap.get("shell_command"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    if !shell.is_empty() {
        return shell.to_string();
    }
    let mut parts = Vec::new();
    if let Some(exe) = cap
        .and_then(|cap| cap.get("executable"))
        .and_then(Value::as_str)
    {
        parts.push(exe.to_string());
    }
    if let Some(args) = cap
        .and_then(|cap| cap.get("arguments"))
        .and_then(Value::as_array)
    {
        parts.extend(args.iter().map(value_as_string));
    }
    parts.join(" ").trim().to_string()
}

fn cwd_from(payload: &Value, body: &serde_json::Map<String, Value>) -> Option<String> {
    let cap = as_object(body.get("capability"));
    if let Some(dir) = cap.and_then(|cap| nonempty_str(cap.get("working_directory"))) {
        return Some(dir.to_string());
    }
    payload
        .get("workspace")
        .and_then(Value::as_object)
        .and_then(|workspace| nonempty_str(workspace.get("root")))
        .map(str::to_string)
}

fn bash_exit_code(
    payload: &serde_json::Map<String, Value>,
    tool_response: Option<&serde_json::Map<String, Value>>,
) -> Option<i64> {
    for source in [Some(payload), tool_response].into_iter().flatten() {
        for key in ["exit_code", "exitCode", "return_code", "returnCode"] {
            if let Some(code) = parse_exit_code(source.get(key)) {
                return Some(code);
            }
        }
    }
    let error_text = payload
        .get("error")
        .map(value_as_string)
        .unwrap_or_default();
    let re = exit_code_re();
    let caps = re.captures(&error_text)?;
    parse_code_int(&caps[1])
}

fn parse_exit_code(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    if value.is_null() {
        return None;
    }
    if let Some(n) = value.as_i64() {
        return valid_code(n);
    }
    if let Some(n) = value.as_u64() {
        return valid_code(n as i64);
    }
    let text = value.as_str()?.trim();
    if text.is_empty() {
        return None;
    }
    parse_code_int(text)
}

fn parse_code_int(text: &str) -> Option<i64> {
    valid_code(text.parse::<i64>().ok()?)
}

fn valid_code(code: i64) -> Option<i64> {
    (0..=255).contains(&code).then_some(code)
}

fn as_object(value: Option<&Value>) -> Option<&serde_json::Map<String, Value>> {
    value.and_then(Value::as_object)
}

fn nonempty_str(value: Option<&Value>) -> Option<&str> {
    value.and_then(Value::as_str).filter(|s| !s.is_empty())
}

fn first_str(obj: Option<&serde_json::Map<String, Value>>, keys: &[&str]) -> String {
    let Some(obj) = obj else {
        return String::new();
    };
    for key in keys {
        if let Some(text) = nonempty_str(obj.get(*key)) {
            return text.to_string();
        }
    }
    String::new()
}

fn boolish(value: Option<&Value>) -> bool {
    match value {
        Some(Value::Bool(true)) => true,
        Some(Value::Number(n)) => n.as_i64() == Some(1),
        Some(Value::String(s)) => s == "true" || s == "1",
        _ => false,
    }
}

fn value_as_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn truncate(text: String) -> String {
    if text.len() <= MAX_OUTPUT {
        text
    } else {
        text.chars().take(MAX_OUTPUT).collect()
    }
}

fn exit_code_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(?:exit(?:\s+|-)?code\s*[:=]?\s*|exited with code\s+)(\d+)")
            .expect("exit-code regex")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn claude(value: Value) -> Option<CommandRecord> {
        record_from_value(&value, Agent::Claude)
    }

    fn rho(value: Value) -> Option<CommandRecord> {
        record_from_value(&value, Agent::Rho)
    }

    #[test]
    fn claude_success_uses_stdout_and_zero_exit() {
        let record = claude(json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "Bash",
            "cwd": "/work",
            "tool_input": {"command": "cargo test"},
            "tool_response": {"stdout": "ok", "stderr": ""}
        }))
        .unwrap();
        assert_eq!(record.command_parts, ["cargo test"]);
        assert_eq!(record.cwd.as_deref(), Some("/work"));
        assert_eq!(record.stdout, "ok");
        assert_eq!(record.exit_code, Some(0));
        assert_eq!(record.notes, "agent:claude-code");
    }

    #[test]
    fn claude_failure_parses_exit_code_from_error_text() {
        let record = claude(json!({
            "hook_event_name": "PostToolUseFailure",
            "tool_name": "Bash",
            "tool_input": {"command": "false"},
            "error": "Exit code 1"
        }))
        .unwrap();
        assert_eq!(record.stderr, "Exit code 1");
        assert_eq!(record.stdout, "");
        assert_eq!(record.exit_code, Some(1));
    }

    #[test]
    fn claude_failure_without_exit_code_omits_it() {
        let record = claude(json!({
            "hook_event_name": "PostToolUseFailure",
            "tool_name": "Bash",
            "tool_input": {"command": "false"},
            "error": "permission denied before launch"
        }))
        .unwrap();
        assert!(record.exit_code.is_none());
    }

    #[test]
    fn claude_ignores_non_bash_tools() {
        assert!(claude(json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "Read",
            "tool_input": {"command": "should-not-record"}
        }))
        .is_none());
    }

    #[test]
    fn rho_failed_call_without_command_uses_tool_name() {
        let record = rho(json!({
            "schema_version": 2,
            "event": "after_tool_use",
            "workspace": {"root": "/work"},
            "payload": {
                "tool": {"name": "bash", "call_id": "call-1"},
                "status": "failed",
                "failure": {"kind": "tool", "message": "exit 1"},
                "duration_ms": 12
            }
        }))
        .unwrap();
        assert_eq!(record.command_parts, ["bash"]);
        assert_eq!(record.cwd.as_deref(), Some("/work"));
        assert_eq!(record.stderr, "tool: exit 1");
        assert_eq!(record.exit_code, Some(1));
        assert_eq!(record.notes, "agent:rho");
    }

    #[test]
    fn rho_reads_shell_command_when_present() {
        let record = rho(json!({
            "event": "after_tool_use",
            "payload": {
                "tool": {"name": "bash"},
                "status": "succeeded",
                "capability": {
                    "working_directory": "/tmp",
                    "executable": "bash",
                    "arguments": ["-lc"],
                    "shell_command": "cargo test"
                }
            }
        }))
        .unwrap();
        assert_eq!(record.command_parts, ["cargo test"]);
        assert_eq!(record.cwd.as_deref(), Some("/tmp"));
        assert_eq!(record.exit_code, Some(0));
    }

    #[test]
    fn rho_skips_successful_calls_without_command() {
        assert!(rho(json!({
            "event": "after_tool_use",
            "payload": {
                "tool": {"name": "bash"},
                "status": "succeeded"
            }
        }))
        .is_none());
    }

    #[test]
    fn named_client_does_not_parse_the_other_payload() {
        let claude_payload = json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "cargo test"}
        });
        assert!(record_from_value(&claude_payload, Agent::Rho).is_none());

        let rho_payload = json!({
            "event": "after_tool_use",
            "payload": {
                "tool": {"name": "bash"},
                "status": "failed",
                "failure": {"kind": "tool", "message": "exit 1"}
            }
        });
        assert!(record_from_value(&rho_payload, Agent::Claude).is_none());
    }
}

// mw-mcp: a Model Context Protocol server over stdio, so an AI agent (Claude
// Code, Codex, Cursor, …) can query your MemoryWhale memory directly instead of
// pasting it in. Speaks newline-delimited JSON-RPC 2.0 on stdin/stdout.
//
// Register with Claude Code:
//   claude mcp add memorywhale -- mw-mcp
//
// Tools exposed: recent_errors, search_memory, get_context.

use rusqlite::{params, Connection};
use serde_json::{json, Value};
use std::io::{BufRead, Write};

fn main() {
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(_) => continue,
        };
        // Notifications (no `id`) get no reply.
        let Some(id) = msg.get("id").cloned() else {
            continue;
        };
        let method = msg.get("method").and_then(Value::as_str).unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or(json!({}));
        let reply = match handle(method, &params) {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err(msg) => json!({"jsonrpc": "2.0", "id": id,
                "error": {"code": -32603, "message": msg}}),
        };
        let _ = writeln!(stdout, "{reply}");
        let _ = stdout.flush();
    }
}

fn handle(method: &str, params: &Value) -> Result<Value, String> {
    match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "memorywhale", "version": env!("CARGO_PKG_VERSION")}
        })),
        "tools/list" => Ok(json!({"tools": tool_defs()})),
        "tools/call" => {
            let name = params.get("name").and_then(Value::as_str).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(json!({}));
            let text = call_tool(name, &args)?;
            Ok(json!({"content": [{"type": "text", "text": text}]}))
        }
        // Unknown method: return empty result rather than erroring the session.
        _ => Ok(json!({})),
    }
}

fn tool_defs() -> Value {
    json!([
        {
            "name": "recent_errors",
            "description": "Recent failed commands (non-zero exit) with their error output. Use this first when debugging a recurring failure.",
            "inputSchema": {"type": "object", "properties": {
                "limit": {"type": "integer", "description": "max results (default 8)"}
            }}
        },
        {
            "name": "search_memory",
            "description": "Search remembered commands, arguments, output, and notes for a term.",
            "inputSchema": {"type": "object", "properties": {
                "query": {"type": "string", "description": "text to search for"}
            }, "required": ["query"]}
        },
        {
            "name": "get_context",
            "description": "A compact digest of recent failed commands and sessions, optionally scoped to a project tag.",
            "inputSchema": {"type": "object", "properties": {
                "project": {"type": "string", "description": "project tag, e.g. project:demo"}
            }}
        }
    ])
}

fn open() -> Result<Connection, String> {
    let path = memorywhale_cli::database_path()?;
    Connection::open(&path).map_err(|e| format!("failed to open {}: {e}", path.display()))
}

fn call_tool(name: &str, args: &Value) -> Result<String, String> {
    match name {
        "recent_errors" => {
            let limit = args.get("limit").and_then(Value::as_i64).unwrap_or(8);
            recent_errors(limit)
        }
        "search_memory" => {
            let q = args
                .get("query")
                .and_then(Value::as_str)
                .ok_or_else(|| "search_memory needs a 'query'".to_string())?;
            search_memory(q)
        }
        "get_context" => {
            let project = args.get("project").and_then(Value::as_str);
            get_context(project)
        }
        other => Err(format!("unknown tool: {other}")),
    }
}

fn recent_errors(limit: i64) -> Result<String, String> {
    let conn = open()?;
    let mut stmt = conn
        .prepare(
            "SELECT argv_json, cwd, exit_code, stderr, notes, created_at
             FROM command_runs
             WHERE exit_code IS NOT NULL AND exit_code != 0
             ORDER BY id DESC LIMIT ?1",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![limit], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut out = String::new();
    for row in rows {
        let (argv_json, cwd, exit_code, stderr, notes, created_at) = row.map_err(|e| e.to_string())?;
        let argv: Vec<String> = serde_json::from_str(&argv_json).unwrap_or_default();
        out.push_str(&format!(
            "- `{}` (exit {}, {})\n  cwd: {}\n  err: {}\n  note: {}\n",
            argv.join(" "),
            exit_code.unwrap_or(-1),
            created_at,
            cwd.unwrap_or_default(),
            last_line(&stderr, 240),
            notes.trim()
        ));
    }
    Ok(if out.is_empty() {
        "(no failed commands recorded)".to_string()
    } else {
        out
    })
}

fn search_memory(query: &str) -> Result<String, String> {
    let conn = open()?;
    let like = format!("%{query}%");
    let mut stmt = conn
        .prepare(
            "SELECT argv_json, exit_code, notes, created_at
             FROM command_runs
             WHERE command LIKE ?1 OR argv_json LIKE ?1 OR stdout LIKE ?1
                OR stderr LIKE ?1 OR notes LIKE ?1
             ORDER BY id DESC LIMIT 20",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![like], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut out = String::new();
    for row in rows {
        let (argv_json, exit_code, notes, created_at) = row.map_err(|e| e.to_string())?;
        let argv: Vec<String> = serde_json::from_str(&argv_json).unwrap_or_default();
        out.push_str(&format!(
            "- `{}` (exit {}, {}){}\n",
            argv.join(" "),
            exit_code.unwrap_or(0),
            created_at,
            if notes.trim().is_empty() {
                String::new()
            } else {
                format!(" — {}", notes.trim())
            }
        ));
    }
    Ok(if out.is_empty() {
        format!("(no matches for {query:?})")
    } else {
        out
    })
}

fn get_context(project: Option<&str>) -> Result<String, String> {
    let conn = open()?;
    let like = project.map(|p| format!("%{p}%"));
    let mut stmt = conn
        .prepare(
            "SELECT argv_json, exit_code, stderr, created_at
             FROM command_runs
             WHERE exit_code IS NOT NULL AND exit_code != 0
               AND (?1 IS NULL OR notes LIKE ?1)
             ORDER BY id DESC LIMIT 8",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![like.as_deref()], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<i64>>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut out = String::from("Recent failed commands:\n");
    let mut any = false;
    for row in rows {
        let (argv_json, exit_code, stderr, created_at) = row.map_err(|e| e.to_string())?;
        let argv: Vec<String> = serde_json::from_str(&argv_json).unwrap_or_default();
        any = true;
        out.push_str(&format!(
            "- `{}` (exit {}, {}): {}\n",
            argv.join(" "),
            exit_code.unwrap_or(-1),
            created_at,
            last_line(&stderr, 200)
        ));
    }
    if !any {
        out.push_str("(none)\n");
    }
    Ok(out)
}

/// Last non-empty line, char-capped (safe on UTF-8).
fn last_line(text: &str, max: usize) -> String {
    let t = text
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    let chars: Vec<char> = t.chars().collect();
    if chars.len() > max {
        format!("…{}", chars[chars.len() - max..].iter().collect::<String>())
    } else {
        t.to_string()
    }
}

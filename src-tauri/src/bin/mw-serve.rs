// mw-serve: serve MemoryWhale's local memory as a web dashboard.
//
// Starts a small HTTP server (no external dependencies) that reads the local
// SQLite store and serves a browsable page of your previous command runs and
// recorded sessions. Designed for headless machines (e.g. a Jetson): run it on
// the machine that has the data, then open it from a laptop browser over the LAN
// at http://<machine-ip>:<port>/. Everything stays local; nothing is uploaded.
//
// Usage:
//   mw-serve                 serve on 0.0.0.0:7071
//   mw-serve --port 8080     serve on a different port
//   mw-serve --host 127.0.0.1  bind to localhost only

use chrono::{DateTime, Utc};
use regex::Regex;
use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn main() {
    if let Err(err) = run() {
        eprintln!("mw-serve: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut host = "0.0.0.0".to_string();
    let mut port: u16 = 7071;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                println!("mw-serve [--host <addr>] [--port <n>]  — serve memory as a web dashboard");
                return Ok(());
            }
            "--host" => host = args.next().unwrap_or(host),
            "--port" => {
                port = args
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or("--port needs a number")?;
            }
            other => return Err(format!("unknown option {other:?}; run mw-serve --help")),
        }
    }

    let db = database_path()?;

    // Self-heal: import any session transcripts whose recording was interrupted
    // before it could write its database row.
    match recover_orphans() {
        Ok(n) if n > 0 => println!("Recovered {n} interrupted session(s) from transcripts."),
        Err(e) => eprintln!("mw-serve: recovery skipped: {e}"),
        _ => {}
    }

    let listener = TcpListener::bind((host.as_str(), port))
        .map_err(|e| format!("failed to bind {host}:{port}: {e}"))?;

    println!("MemoryWhale dashboard serving from {}", db.display());
    println!("  local:   http://localhost:{port}/");
    if host == "0.0.0.0" {
        println!("  network: http://<this-machine-ip>:{port}/  (find it with: hostname -I)");
    }
    println!("Press Ctrl-C to stop.");

    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                std::thread::spawn(move || handle(s));
            }
            Err(e) => eprintln!("mw-serve: connection error: {e}"),
        }
    }
    Ok(())
}

fn handle(mut stream: TcpStream) {
    let read_stream = match stream.try_clone() {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut reader = BufReader::new(read_stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let raw_path = request_line.split_whitespace().nth(1).unwrap_or("/");
    let path = raw_path.split('?').next().unwrap_or("/");

    let (status, body) = route(path);
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.as_bytes().len()
    );
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.write_all(body.as_bytes());
}

fn route(path: &str) -> (&'static str, String) {
    if path == "/" {
        return ("200 OK", dashboard());
    }
    if path == "/favicon.ico" {
        return ("204 No Content", String::new());
    }
    if let Some(rest) = path.strip_prefix("/command/") {
        if let Ok(id) = rest.parse::<i64>() {
            return match command_page(id) {
                Ok(html) => ("200 OK", html),
                Err(e) => ("404 Not Found", page("Not found", &format!("<p>{}</p>", esc(&e)))),
            };
        }
    }
    if let Some(rest) = path.strip_prefix("/session/") {
        if let Ok(id) = rest.parse::<i64>() {
            return match session_page(id) {
                Ok(html) => ("200 OK", html),
                Err(e) => ("404 Not Found", page("Not found", &format!("<p>{}</p>", esc(&e)))),
            };
        }
    }
    ("404 Not Found", page("Not found", "<p>Nothing here. <a href=\"/\">Back to dashboard</a></p>"))
}

fn dashboard() -> String {
    let conn = match open_db() {
        Ok(c) => c,
        Err(e) => return page("MemoryWhale", &format!("<p>Could not open database: {}</p>", esc(&e))),
    };
    let _ = init_min_schema(&conn);

    let mut body = String::from("<div class=\"eyebrow\">MemoryWhale</div>\n<h1>Terminal memory</h1>\n");
    body.push_str("<p class=\"sub\">Your previous commands and recorded sessions, served locally.</p>\n");

    body.push_str("<h2>Command runs</h2>\n<div class=\"list\">\n");
    let mut rows = 0;
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, command, exit_code, created_at, notes FROM command_runs ORDER BY id DESC LIMIT 200",
    ) {
        if let Ok(iter) = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
            ))
        }) {
            for row in iter.flatten() {
                let (id, cmd, code, at, notes) = row;
                let ok = code == Some(0);
                body.push_str(&format!(
                    "<a class=\"row\" href=\"/command/{id}\"><span class=\"badge {}\">{}</span>\
                     <span class=\"cmd\">{}</span><span class=\"when\">{}</span><span class=\"note\">{}</span></a>\n",
                    if ok { "ok" } else { "bad" },
                    match code { Some(c) => format!("exit {c}"), None => "—".into() },
                    esc(&cmd),
                    esc(&at),
                    esc(&notes)
                ));
                rows += 1;
            }
        }
    }
    if rows == 0 {
        body.push_str("<p class=\"empty\">No command runs yet. Record one with <code>mw-remember</code>.</p>\n");
    }
    body.push_str("</div>\n");

    body.push_str("<h2>Sessions</h2>\n<div class=\"list\">\n");
    let mut srows = 0;
    if let Ok(mut stmt) = conn.prepare(
        "SELECT id, started_at, byte_count, notes FROM sessions ORDER BY id DESC LIMIT 200",
    ) {
        if let Ok(iter) = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
            ))
        }) {
            for row in iter.flatten() {
                let (id, at, bytes, notes) = row;
                body.push_str(&format!(
                    "<a class=\"row\" href=\"/session/{id}\"><span class=\"badge sess\">session</span>\
                     <span class=\"cmd\">#{id}</span><span class=\"when\">{}</span><span class=\"note\">{} · {bytes} bytes</span></a>\n",
                    esc(&at),
                    esc(&notes)
                ));
                srows += 1;
            }
        }
    }
    if srows == 0 {
        body.push_str("<p class=\"empty\">No sessions yet. Record one with <code>mw</code>.</p>\n");
    }
    body.push_str("</div>\n");

    page("MemoryWhale — terminal memory", &body)
}

fn command_page(id: i64) -> Result<String, String> {
    let conn = open_db()?;
    let row = conn
        .query_row(
            "SELECT command, argv_json, cwd, exit_code, stdout, stderr, notes, created_at
             FROM command_runs WHERE id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, Option<String>>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                    r.get::<_, String>(4)?,
                    r.get::<_, String>(5)?,
                    r.get::<_, String>(6)?,
                    r.get::<_, String>(7)?,
                ))
            },
        )
        .optional()
        .map_err(|e| format!("read command run: {e}"))?
        .ok_or_else(|| format!("no command run #{id}"))?;
    let (command, argv_json, cwd, exit_code, stdout, stderr, notes, created_at) = row;
    let argv: Vec<String> = serde_json::from_str(&argv_json).unwrap_or_else(|_| vec![command.clone()]);
    let ok = exit_code == Some(0);

    let mut body = String::from("<a class=\"back\" href=\"/\">← all memory</a>\n");
    body.push_str(&format!("<div class=\"eyebrow\">command run · #{id}</div>\n<h1>{}</h1>\n", esc(&command)));
    body.push_str(&format!(
        "<div class=\"badge {}\">{}</div>\n",
        if ok { "ok" } else { "bad" },
        match exit_code { Some(0) => "exit 0 · success".to_string(), Some(c) => format!("exit {c} · failed"), None => "no exit code".to_string() }
    ));
    body.push_str("<div class=\"meta\">");
    if let Some(cwd) = &cwd { body.push_str(&format!("<div><span>cwd</span>{}</div>", esc(cwd))); }
    body.push_str(&format!("<div><span>when</span>{}</div></div>\n", esc(&created_at)));

    body.push_str("<h2>Command</h2>\n");
    body.push_str(&code_block(&argv.join(" ")));
    if !stdout.trim().is_empty() {
        body.push_str("<h2>Output</h2>\n");
        body.push_str(&format!("<pre class=\"out\">{}</pre>\n", esc(&stdout)));
    }
    if !stderr.trim().is_empty() {
        body.push_str("<h2>Error log</h2>\n");
        body.push_str(&format!("<pre class=\"err\">{}</pre>\n", esc(&stderr)));
    }
    if !notes.trim().is_empty() {
        body.push_str(&format!("<h2>Note</h2>\n<p class=\"noteblock\">{}</p>\n", esc(&notes)));
    }
    body.push_str(&hints(&conn, id, &command, ok));
    Ok(page(&format!("{} · MemoryWhale", command), &body))
}

fn session_page(id: i64) -> Result<String, String> {
    let conn = open_db()?;
    let row = conn
        .query_row(
            "SELECT shell, cwd, notes, started_at, byte_count, transcript FROM sessions WHERE id = ?1",
            params![id],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?,
                    r.get::<_, Option<String>>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, i64>(4)?,
                    r.get::<_, String>(5)?,
                ))
            },
        )
        .optional()
        .map_err(|e| format!("read session: {e}"))?
        .ok_or_else(|| format!("no session #{id}"))?;
    let (shell, cwd, notes, started_at, byte_count, transcript) = row;

    let mut body = String::from("<a class=\"back\" href=\"/\">← all memory</a>\n");
    body.push_str(&format!("<div class=\"eyebrow\">recorded session · #{id}</div>\n<h1>Session {id}</h1>\n"));
    body.push_str("<div class=\"meta\">");
    if let Some(shell) = &shell { body.push_str(&format!("<div><span>shell</span>{}</div>", esc(shell))); }
    if let Some(cwd) = &cwd { body.push_str(&format!("<div><span>cwd</span>{}</div>", esc(cwd))); }
    body.push_str(&format!("<div><span>started</span>{}</div>", esc(&started_at)));
    body.push_str(&format!("<div><span>size</span>{byte_count} bytes</div></div>\n"));
    if !notes.trim().is_empty() {
        body.push_str(&format!("<p class=\"noteblock\">{}</p>\n", esc(&notes)));
    }
    body.push_str("<h2>Transcript</h2>\n");
    body.push_str(&format!("<pre class=\"out\">{}</pre>\n", esc(&transcript)));
    Ok(page(&format!("Session {id} · MemoryWhale"), &body))
}

fn hints(conn: &Connection, id: i64, command: &str, ok: bool) -> String {
    let mut out = String::new();
    let mut items: Vec<(String, Option<String>)> = Vec::new();

    if let Ok(total) = conn.query_row(
        "SELECT COUNT(*) FROM command_runs WHERE command = ?1",
        params![command],
        |r| r.get::<_, i64>(0),
    ) {
        if total > 1 {
            let failures: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM command_runs WHERE command = ?1 AND exit_code <> 0",
                    params![command],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            items.push((
                format!("You've run `{command}` {total} time(s) — {} succeeded, {failures} failed.", total - failures),
                None,
            ));
        }
    }

    if !ok {
        if let Ok(Some(argv_json)) = conn
            .query_row(
                "SELECT argv_json FROM command_runs WHERE command = ?1 AND exit_code = 0 AND id <> ?2 ORDER BY created_at DESC LIMIT 1",
                params![command, id],
                |r| r.get::<_, String>(0),
            )
            .optional()
        {
            let argv: Vec<String> = serde_json::from_str(&argv_json).unwrap_or_default();
            if !argv.is_empty() {
                items.push((format!("A previous run of `{command}` succeeded — try that exact command:"), Some(argv.join(" "))));
            }
        }
        if let Ok(Some(prev_at)) = conn
            .query_row(
                "SELECT created_at FROM command_runs WHERE command = ?1 AND exit_code <> 0 AND id <> ?2 ORDER BY created_at DESC LIMIT 1",
                params![command, id],
                |r| r.get::<_, String>(0),
            )
            .optional()
        {
            if let Ok(Some((next_cmd, next_argv))) = conn
                .query_row(
                    "SELECT command, argv_json FROM command_runs WHERE created_at > ?1 ORDER BY created_at ASC LIMIT 1",
                    params![prev_at],
                    |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
                )
                .optional()
            {
                let argv: Vec<String> = serde_json::from_str(&next_argv).unwrap_or_default();
                let line = if argv.is_empty() { next_cmd } else { argv.join(" ") };
                items.push(("Last time this command failed, the next thing you ran was:".to_string(), Some(line)));
            }
        }
    }

    if items.is_empty() {
        return out;
    }
    out.push_str("<h2>Suggested next steps</h2>\n<div class=\"hints\">\n");
    for (text, snippet) in items {
        out.push_str("<div class=\"hint\"><p>");
        out.push_str(&esc(&text));
        out.push_str("</p>");
        if let Some(s) = snippet {
            out.push_str(&code_block(&s));
        }
        out.push_str("</div>\n");
    }
    out.push_str("</div>\n");
    out
}

fn code_block(text: &str) -> String {
    format!("<div class=\"codeblock\"><code>{}</code></div>\n", esc(text))
}

fn page(title: &str, body: &str) -> String {
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"/>\
<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"/>\
<title>{}</title>\n<style>{CSS}</style></head>\n<body><main>{body}\
<footer>MemoryWhale — served locally from SQLite · nothing is uploaded</footer></main></body></html>\n",
        esc(title)
    )
}

const CSS: &str = r#"
:root{--ink:#0f1722;--muted:#566273;--line:#e5ebf2;--azure:#2b43dd;--cyan:#10b6c6;--ok:#168a69;--bad:#e9663a;--bg:#f3f7fb;--card:#fff;}
*{box-sizing:border-box}
body{margin:0;background:var(--bg);color:var(--ink);font-family:"Hanken Grotesk",system-ui,-apple-system,"Segoe UI",sans-serif;line-height:1.55}
main{max-width:920px;margin:0 auto;padding:40px 24px 80px}
a{color:inherit;text-decoration:none}
.eyebrow{font:600 .72rem/1 ui-monospace,monospace;letter-spacing:.16em;text-transform:uppercase;color:var(--azure);margin-bottom:10px}
.back{display:inline-block;margin-bottom:18px;color:var(--azure);font:600 .8rem ui-monospace,monospace}
h1{font-size:2rem;margin:.1em 0 .3em;letter-spacing:-.02em}
h2{font-size:.95rem;margin:1.8em 0 .6em;text-transform:uppercase;letter-spacing:.08em;color:var(--muted)}
.sub{color:var(--muted);margin:0 0 1em}
.list{display:flex;flex-direction:column;gap:8px}
.row{display:grid;grid-template-columns:90px 1fr 1.2fr 1.4fr;gap:14px;align-items:center;background:var(--card);border:1px solid var(--line);border-radius:10px;padding:12px 16px;transition:border-color .15s}
.row:hover{border-color:var(--azure)}
.row .cmd{font:600 .95rem ui-monospace,monospace}
.row .when{font:.78rem ui-monospace,monospace;color:var(--muted)}
.row .note{font-size:.85rem;color:var(--muted);overflow:hidden;text-overflow:ellipsis;white-space:nowrap}
.badge{display:inline-block;font:600 .72rem ui-monospace,monospace;padding:4px 10px;border-radius:999px;text-align:center}
.badge.ok{background:#e6f6ef;color:var(--ok)}
.badge.bad{background:#fceee7;color:var(--bad)}
.badge.sess{background:#eaeefe;color:var(--azure)}
.meta{display:flex;flex-wrap:wrap;gap:8px 24px;margin:16px 0;font-size:.9rem;color:var(--muted)}
.meta span{display:block;font:600 .7rem ui-monospace,monospace;text-transform:uppercase;letter-spacing:.08em;color:var(--azure)}
pre{background:#0b1c25;color:#e3f2f4;padding:16px;border-radius:10px;overflow:auto;font:.85rem/1.5 ui-monospace,monospace;white-space:pre-wrap;word-break:break-word}
pre.err{color:#ffd9c9}
.noteblock{background:var(--card);border:1px solid var(--line);border-left:3px solid var(--cyan);padding:12px 16px;border-radius:8px}
.codeblock{background:var(--card);border:1px solid var(--line);border-radius:8px;padding:10px 12px;margin:8px 0}
.codeblock code{font:.9rem ui-monospace,monospace;white-space:pre-wrap;word-break:break-word}
.hints{display:flex;flex-direction:column;gap:10px}
.hint{background:var(--card);border:1px solid var(--line);border-radius:10px;padding:14px 16px}
.hint p{margin:0 0 6px}
.empty{color:var(--muted)}
footer{margin-top:60px;padding-top:20px;border-top:1px solid var(--line);font:.75rem ui-monospace,monospace;color:var(--muted)}
"#;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&#39;")
}

fn init_min_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS command_runs (id INTEGER PRIMARY KEY, command TEXT NOT NULL,
            argv_json TEXT NOT NULL, cwd TEXT, exit_code INTEGER, stdout TEXT NOT NULL DEFAULT '',
            stderr TEXT NOT NULL DEFAULT '', notes TEXT NOT NULL DEFAULT '', created_at TEXT NOT NULL);
         CREATE TABLE IF NOT EXISTS sessions (id INTEGER PRIMARY KEY, shell TEXT, cwd TEXT,
            transcript_path TEXT NOT NULL DEFAULT '', transcript TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '', started_at TEXT NOT NULL DEFAULT '',
            ended_at TEXT NOT NULL DEFAULT '', byte_count INTEGER NOT NULL DEFAULT 0);",
    )
    .map_err(|e| format!("init schema: {e}"))
}

/// Import every session `.log` that has no row yet (interrupted recordings).
fn recover_orphans() -> Result<usize, String> {
    let sessions_dir = data_base()?.join("MemoryWhale").join("sessions");
    if !sessions_dir.exists() {
        return Ok(0);
    }
    let conn = open_db()?;
    init_min_schema(&conn)?;

    let mut entries: Vec<PathBuf> = fs::read_dir(&sessions_dir)
        .map_err(|e| format!("read sessions dir: {e}"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "log").unwrap_or(false))
        .collect();
    entries.sort();

    let mut recovered = 0;
    for path in entries {
        let path_str = match path.to_str() {
            Some(s) => s.to_string(),
            None => continue,
        };
        let already: i64 = conn
            .query_row("SELECT COUNT(*) FROM sessions WHERE transcript_path = ?1", params![path_str], |r| r.get(0))
            .unwrap_or(0);
        if already > 0 {
            continue;
        }
        let raw = match fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let cleaned = clean_transcript(&String::from_utf8_lossy(&raw));
        let started = started_from_filename(&path).unwrap_or_else(|| mtime_rfc3339(&path));
        let ended = mtime_rfc3339(&path);
        conn.execute(
            "INSERT INTO sessions (shell, cwd, transcript_path, transcript, notes, started_at, ended_at, byte_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                Option::<String>::None, Option::<String>::None, path_str, cleaned,
                "recovered from transcript (recording was interrupted before saving)",
                started, ended, raw.len() as i64
            ],
        )
        .map_err(|e| format!("insert recovered session: {e}"))?;
        recovered += 1;
    }
    Ok(recovered)
}

fn started_from_filename(path: &Path) -> Option<String> {
    let stamp = path.file_stem()?.to_str()?.strip_prefix("session-")?;
    let re = Regex::new(r"^(\d{4}-\d{2}-\d{2})T(\d{2})-(\d{2})-(\d{2})(\.\d+)?([+-]\d{2})-(\d{2})$").ok()?;
    let c = re.captures(stamp)?;
    Some(format!(
        "{}T{}:{}:{}{}{}:{}",
        &c[1], &c[2], &c[3], &c[4], c.get(5).map(|m| m.as_str()).unwrap_or(""), &c[6], &c[7]
    ))
}

fn mtime_rfc3339(path: &Path) -> String {
    let t = fs::metadata(path).and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);
    DateTime::<Utc>::from(t).to_rfc3339()
}

fn clean_transcript(input: &str) -> String {
    let osc = Regex::new(r"\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)").unwrap();
    let csi = Regex::new(r"\x1b[@-Z\\-_]|\x1b\[[0-?]*[ -/]*[@-~]").unwrap();
    let ctrl = Regex::new(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]").unwrap();
    let s = osc.replace_all(input, "");
    let s = csi.replace_all(&s, "");
    let s = s.replace('\r', "");
    ctrl.replace_all(&s, "").into_owned()
}

fn open_db() -> Result<Connection, String> {
    let path = database_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create data dir: {e}"))?;
    }
    Connection::open(&path).map_err(|e| format!("open db {}: {e}", path.display()))
}

fn database_path() -> Result<PathBuf, String> {
    Ok(data_base()?.join("MemoryWhale").join("memorywhale.sqlite3"))
}

fn data_base() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "could not resolve local data directory".to_string())
}

// mw: automatic whole-session terminal recorder for MemoryWhale.
//
// Starts your $SHELL inside a recorded subshell (via the system `script` tool),
// captures every command and all output until you `exit`, then stores the
// session in the same local SQLite DB as mw-remember:
//   <data_local>/MemoryWhale/memorywhale.sqlite3   (sessions table + cleaned transcript)
//   <data_local>/MemoryWhale/sessions/             (raw transcript files)
//
// Everything is stored locally and never uploaded.
//
// Usage:
//   mw                                  # record a session, exit the subshell to stop
//   mw --notes "debugging the Jetson build"

use chrono::Utc;
use regex::Regex;
use rusqlite::{params, Connection};
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

const LIVE_SYNC_INTERVAL_SECS: u64 = 2;

fn main() {
    if let Err(err) = run() {
        eprintln!("mw: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let raw_args: Vec<String> = env::args().skip(1).collect();
    match raw_args.first().map(String::as_str) {
        Some("show") => return show_session(&raw_args[1..]),
        Some("list") => return list_sessions(),
        Some("mark") => return mark_bookmark(&raw_args[1..]),
        Some("replay") => return replay_command(&raw_args[1..]),
        Some("demo") => return seed_demo(),
        Some("export") => return export_memory(&raw_args[1..]),
        Some("context") => return context_cmd(&raw_args[1..]),
        Some("doctor") => return doctor(),
        Some("global") => return global_cmd(&raw_args[1..]),
        Some("--help") | Some("-h") => {
            print_help();
            return Ok(());
        }
        _ => {}
    }

    if raw_args.is_empty() {
        first_run_welcome()?;
    }

    let mut notes = String::new();
    let mut live = false;
    let mut iter = raw_args.into_iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--notes" => notes = iter.next().unwrap_or_default(),
            "--live" | "--autosave" => live = true,
            value if value.starts_with("--") => {
                return Err(format!("unknown option {value:?}; run mw --help"));
            }
            value => return Err(format!("unexpected argument {value:?}; run mw --help")),
        }
    }
    record_session(append_environment_tags(notes), live)
}

fn record_session(notes: String, live: bool) -> Result<(), String> {
    let shell = env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
    let cwd = env::current_dir()
        .ok()
        .and_then(|path| path.to_str().map(ToOwned::to_owned));

    let started_at = Utc::now().to_rfc3339();
    let sessions_dir = sessions_dir()?;
    fs::create_dir_all(&sessions_dir)
        .map_err(|err| format!("failed to create sessions dir: {err}"))?;
    let transcript_path =
        sessions_dir.join(format!("session-{}.log", started_at.replace(':', "-")));
    let transcript_str = transcript_path
        .to_str()
        .ok_or_else(|| "transcript path is not valid UTF-8".to_string())?
        .to_string();

    eprintln!("mw: recording session to {transcript_str}");
    if live {
        eprintln!(
            "mw: live autosave is on; the dashboard/SQLite row updates every {LIVE_SYNC_INTERVAL_SECS}s."
        );
    }
    eprintln!("mw: type `exit` (or Ctrl-D) to stop recording.\n");

    let live_session = if live {
        let id = insert_live_session(&SessionDraft {
            shell: &shell,
            cwd: cwd.as_deref(),
            transcript_path: &transcript_str,
            notes: &notes,
            started_at: &started_at,
        })?;
        let sync = start_live_sync(id, transcript_path.clone());
        Some((id, sync))
    } else {
        None
    };

    // `script -q <file>` runs $SHELL interactively and records all I/O to <file>
    // on both macOS (BSD script) and Linux (util-linux script). MW_RECORDING is
    // set so the recorded shell's global-recording hook sees the guard and does
    // not start a nested recording, however this session was launched.
    let mut script = Command::new("script");
    script.arg("-q");
    if live && env::consts::OS == "linux" {
        script.arg("-f");
    }
    let status = script
        .arg(&transcript_path)
        .env("MW_RECORDING", "1")
        .status()
        .map_err(|err| format!("failed to launch `script` (is it installed?): {err}"))?;

    let ended_at = Utc::now().to_rfc3339();
    let live_session = if let Some((id, sync)) = live_session {
        sync.stop.store(true, Ordering::SeqCst);
        let _ = sync.handle.join();
        Some(id)
    } else {
        None
    };

    if !transcript_path.exists() {
        return Err("recording produced no transcript (session not saved)".to_string());
    }
    let (id, byte_count) = if let Some(id) = live_session {
        let byte_count =
            update_session_from_transcript(id, &transcript_path, &ended_at, "finished")?;
        (id, byte_count)
    } else {
        insert_finished_session(
            &SessionDraft {
                shell: &shell,
                cwd: cwd.as_deref(),
                transcript_path: &transcript_str,
                notes: &notes,
                started_at: &started_at,
            },
            &transcript_path,
            &ended_at,
        )?
    };

    let exit_note = match status.code() {
        Some(code) => format!("shell exited with code {code}"),
        None => "shell terminated by signal".to_string(),
    };
    eprintln!("\nmw: recorded session #{id} ({byte_count} bytes, {exit_note}) -> {transcript_str}");
    Ok(())
}

fn print_help() {
    println!(
        "mw [--notes <text>]      record a whole shell session until you exit\n\
         mw --live [--notes <text>]  autosave the session to SQLite while it is still running\n\
         mw list                  list recorded sessions\n\
         mw show <id>             print the full faithful transcript of a session\n\
         mw mark <text>           bookmark the current debugging moment\n\
         mw replay <run-id>       rerun a saved command from command_runs\n\
         mw demo                  seed a small demo terminal-memory dataset\n\
         mw export [project:name] export memory to Markdown + JSON\n\
         mw context [project:name] [--last-error] [--limit N]  print a compact digest to paste into an AI agent\n\
         mw doctor                check the install: data dir, database, `script`, and hook status\n\
         mw global on|off|status  auto-record every new terminal by wiring a shell startup hook\n\
         \n\
         Records every command + output, stored locally and never uploaded.\n\
         Raw transcript: <data_local>/MemoryWhale/sessions/\n\
         Metadata + cleaned transcript: <data_local>/MemoryWhale/memorywhale.sqlite3 (sessions table)"
    );
}

/// Shown only on a genuine cold start: no hook wired and nothing recorded yet.
/// Explains `mw` and offers to enable auto-recording; on "no" it falls through
/// to recording this one session so bare `mw` still works as documented.
fn first_run_welcome() -> Result<(), String> {
    use std::io::{IsTerminal, Write};

    // Existing user or scripted call → behave exactly as before (record).
    if global_enabled_path().map(|p| p.exists()).unwrap_or(false) {
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        return Ok(());
    }
    let recorded_before = open_session_db()
        .and_then(|conn| {
            conn.query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get::<_, i64>(0))
                .map_err(|err| err.to_string())
        })
        .map(|count| count > 0)
        .unwrap_or(false);
    if recorded_before {
        return Ok(());
    }

    println!(
        "🐬 Welcome to MemoryWhale.\n\
         \n\
         It records your terminal commands, output, and errors into a local\n\
         SQLite database so debugging context survives crashes, SSH drops, and\n\
         switching machines. Nothing is ever uploaded.\n\
         \n\
         The easiest way to use it is to auto-record every new terminal — no\n\
         need to type `mw` each time. This adds one line to your shell startup\n\
         file (`mw global off` undoes it).\n"
    );
    print!("Enable auto-recording in every new terminal now? [y/N] ");
    let _ = std::io::stdout().flush();

    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .map_err(|err| format!("failed to read input: {err}"))?;

    if matches!(answer.trim(), "y" | "Y" | "yes" | "Yes") {
        global_on()?;
        std::process::exit(0);
    }

    println!("\nNo problem — recording just this one session. Type `exit` to stop.");
    println!("Run `mw --help` to see everything, or `mw global on` later.\n");
    Ok(())
}

struct SessionDraft<'a> {
    shell: &'a str,
    cwd: Option<&'a str>,
    transcript_path: &'a str,
    notes: &'a str,
    started_at: &'a str,
}

struct LiveSync {
    stop: Arc<AtomicBool>,
    handle: thread::JoinHandle<()>,
}

fn insert_live_session(draft: &SessionDraft<'_>) -> Result<i64, String> {
    let conn = open_session_db()?;
    conn.execute(
        "
        INSERT INTO sessions
            (shell, cwd, transcript_path, transcript, notes, started_at, ended_at, byte_count, status)
        VALUES (?1, ?2, ?3, '', ?4, ?5, ?5, 0, 'recording')
        ",
        params![
            draft.shell,
            draft.cwd,
            draft.transcript_path,
            draft.notes,
            draft.started_at
        ],
    )
    .map_err(|err| format!("failed to create live session row: {err}"))?;
    Ok(conn.last_insert_rowid())
}

fn insert_finished_session(
    draft: &SessionDraft<'_>,
    transcript_path: &PathBuf,
    ended_at: &str,
) -> Result<(i64, i64), String> {
    let raw =
        fs::read(transcript_path).map_err(|err| format!("failed to read transcript: {err}"))?;
    let byte_count = raw.len() as i64;
    let cleaned = clean_transcript(&String::from_utf8_lossy(&raw));
    let conn = open_session_db()?;
    conn.execute(
        "
        INSERT INTO sessions
            (shell, cwd, transcript_path, transcript, notes, started_at, ended_at, byte_count, status)
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'finished')
        ",
        params![
            draft.shell,
            draft.cwd,
            draft.transcript_path,
            cleaned,
            draft.notes,
            draft.started_at,
            ended_at,
            byte_count
        ],
    )
    .map_err(|err| format!("failed to insert session: {err}"))?;
    Ok((conn.last_insert_rowid(), byte_count))
}

fn start_live_sync(id: i64, transcript_path: PathBuf) -> LiveSync {
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let handle = thread::spawn(move || {
        while !thread_stop.load(Ordering::SeqCst) {
            thread::sleep(Duration::from_secs(LIVE_SYNC_INTERVAL_SECS));
            if thread_stop.load(Ordering::SeqCst) {
                break;
            }
            let ended_at = Utc::now().to_rfc3339();
            let _ = update_session_from_transcript(id, &transcript_path, &ended_at, "recording");
        }
    });
    LiveSync { stop, handle }
}

fn update_session_from_transcript(
    id: i64,
    transcript_path: &PathBuf,
    ended_at: &str,
    status: &str,
) -> Result<i64, String> {
    let raw = match fs::read(transcript_path) {
        Ok(raw) => raw,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(err) => return Err(format!("failed to read transcript: {err}")),
    };
    let byte_count = raw.len() as i64;
    let cleaned = clean_transcript(&String::from_utf8_lossy(&raw));
    let conn = open_session_db()?;
    conn.execute(
        "
        UPDATE sessions
        SET transcript = ?1, ended_at = ?2, byte_count = ?3, status = ?4
        WHERE id = ?5
        ",
        params![cleaned, ended_at, byte_count, status, id],
    )
    .map_err(|err| format!("failed to autosave session: {err}"))?;
    Ok(byte_count)
}

fn open_session_db() -> Result<Connection, String> {
    let db_path = database_path()?;
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("failed to create data dir: {err}"))?;
    }
    let conn = Connection::open(db_path).map_err(|err| format!("failed to open db: {err}"))?;
    init_schema(&conn)?;
    Ok(conn)
}

fn mark_bookmark(args: &[String]) -> Result<(), String> {
    if args.is_empty() {
        return Err("usage: mw mark <text>".to_string());
    }
    let label = args.join(" ");
    let cwd = env::current_dir()
        .ok()
        .and_then(|path| path.to_str().map(ToOwned::to_owned));
    let conn = open_session_db()?;
    conn.execute(
        "INSERT INTO bookmarks (label, cwd, created_at) VALUES (?1, ?2, ?3)",
        params![label, cwd, Utc::now().to_rfc3339()],
    )
    .map_err(|err| format!("failed to save bookmark: {err}"))?;
    println!("mw: marked bookmark #{}", conn.last_insert_rowid());
    Ok(())
}

fn replay_command(args: &[String]) -> Result<(), String> {
    let id: i64 = args
        .first()
        .ok_or_else(|| "usage: mw replay <command-run-id>".to_string())?
        .parse()
        .map_err(|_| "command-run-id must be a number".to_string())?;
    let conn = open_session_db()?;
    let (argv_json, cwd): (String, Option<String>) = conn
        .query_row(
            "SELECT argv_json, cwd FROM command_runs WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|err| format!("failed to read command run #{id}: {err}"))?;
    let argv: Vec<String> =
        serde_json::from_str(&argv_json).map_err(|err| format!("bad stored argv: {err}"))?;
    if argv.is_empty() {
        return Err(format!("command run #{id} has no argv"));
    }

    println!("mw: replaying #{}: {}", id, argv.join(" "));
    let mut command = Command::new(&argv[0]);
    command.args(&argv[1..]);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let status = command
        .status()
        .map_err(|err| format!("failed to replay command: {err}"))?;
    println!("mw: replay exited with {}", status);
    Ok(())
}

fn seed_demo() -> Result<(), String> {
    let conn = open_session_db()?;
    let now = Utc::now().to_rfc3339();
    let demo_notes = "project:demo host:jetson runtime:host";
    conn.execute(
        "INSERT INTO command_runs (command, argv_json, cwd, exit_code, stdout, stderr, notes, created_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            "cargo",
            serde_json::to_string(&vec!["cargo", "check"]).unwrap(),
            "/demo/MemoryWhale",
            101_i64,
            "",
            "error: failed to build\\nNo package 'libsoup-3.0' found\\n",
            demo_notes,
            now
        ],
    )
    .map_err(|err| format!("failed to insert demo command: {err}"))?;
    let run_id = conn.last_insert_rowid();
    for (position, value) in ["cargo", "check"].iter().enumerate() {
        conn.execute(
            "INSERT INTO command_arguments (command_run_id, position, value) VALUES (?1, ?2, ?3)",
            params![run_id, position as i64, value],
        )
        .map_err(|err| format!("failed to insert demo argument: {err}"))?;
    }
    conn.execute(
        "INSERT INTO bookmarks (label, cwd, created_at) VALUES (?1, ?2, ?3)",
        params![
            "Tauri build failed here; install missing Linux packages.",
            "/demo/MemoryWhale",
            Utc::now().to_rfc3339()
        ],
    )
    .map_err(|err| format!("failed to insert demo bookmark: {err}"))?;
    println!("mw: demo memory inserted. Run `mw-serve` and search for project:demo.");
    Ok(())
}

fn export_memory(args: &[String]) -> Result<(), String> {
    let project = args.first().cloned();
    let export_dir = memorywhale_dir()?.join("exports");
    fs::create_dir_all(&export_dir)
        .map_err(|err| format!("failed to create exports dir: {err}"))?;
    let stamp = Utc::now().format("%Y%m%d-%H%M%S").to_string();
    let base = project
        .as_deref()
        .unwrap_or("all")
        .replace([':', '/', '\\', ' '], "-");
    let bundle_dir = export_dir.join(format!("{base}-{stamp}"));
    let transcripts_dir = bundle_dir.join("transcripts");
    fs::create_dir_all(&transcripts_dir)
        .map_err(|err| format!("failed to create bundle dir: {err}"))?;
    let markdown_path = bundle_dir.join("memory.md");
    let json_path = bundle_dir.join("memory.json");
    let sqlite_path = bundle_dir.join("memorywhale.sqlite3");
    let conn = open_session_db()?;
    let like = project.as_ref().map(|p| format!("%{p}%"));

    let mut md = String::from("# MemoryWhale Debug Bundle\n\n");
    let mut commands = Vec::new();
    let mut sessions = Vec::new();
    let mut stmt = conn
        .prepare(
            "SELECT id, command, argv_json, cwd, exit_code, stdout, stderr, notes, created_at
             FROM command_runs
             WHERE ?1 IS NULL OR notes LIKE ?1
             ORDER BY id",
        )
        .map_err(|err| format!("failed to prepare command export: {err}"))?;
    let rows = stmt
        .query_map(params![like.as_deref()], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, String>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, String>(8)?,
            ))
        })
        .map_err(|err| format!("failed to export commands: {err}"))?;
    for row in rows {
        let (id, command, argv_json, cwd, exit_code, stdout, stderr, notes, created_at) =
            row.map_err(|err| format!("command row error: {err}"))?;
        let argv: Vec<String> = serde_json::from_str(&argv_json).unwrap_or_default();
        md.push_str(&format!(
            "## Command #{id}: `{}`\n\n- when: `{created_at}`\n- cwd: `{}`\n- exit: `{:?}`\n- notes: {}\n\n```text\n{}\n{}\n```\n\n",
            argv.join(" "),
            cwd.clone().unwrap_or_default(),
            exit_code,
            notes,
            stdout,
            stderr
        ));
        commands.push(serde_json::json!({
            "id": id,
            "command": command,
            "argv": argv,
            "cwd": cwd,
            "exit_code": exit_code,
            "stdout": stdout,
            "stderr": stderr,
            "notes": notes,
            "created_at": created_at
        }));
    }

    let mut stmt = conn
        .prepare(
            "SELECT id, transcript_path, transcript, notes, started_at, ended_at, byte_count, status
             FROM sessions
             WHERE ?1 IS NULL OR notes LIKE ?1
             ORDER BY id",
        )
        .map_err(|err| format!("failed to prepare session export: {err}"))?;
    let rows = stmt
        .query_map(params![like.as_deref()], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
                r.get::<_, i64>(6)?,
                r.get::<_, String>(7)?,
            ))
        })
        .map_err(|err| format!("failed to export sessions: {err}"))?;
    for row in rows {
        let (id, transcript_path, transcript, notes, started_at, ended_at, byte_count, status) =
            row.map_err(|err| format!("session row error: {err}"))?;
        let transcript_file = transcripts_dir.join(format!("session-{id}.txt"));
        fs::write(&transcript_file, &transcript)
            .map_err(|err| format!("failed to write transcript export: {err}"))?;
        md.push_str(&format!(
            "## Session #{id}\n\n- started: `{started_at}`\n- ended: `{ended_at}`\n- status: `{status}`\n- bytes: `{byte_count}`\n- notes: {notes}\n- transcript: `{}`\n\n",
            transcript_file
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("transcript.txt")
        ));
        sessions.push(serde_json::json!({
            "id": id,
            "transcript_path": transcript_path,
            "exported_transcript": transcript_file.file_name().and_then(|name| name.to_str()).unwrap_or("transcript.txt"),
            "notes": notes,
            "started_at": started_at,
            "ended_at": ended_at,
            "byte_count": byte_count,
            "status": status
        }));
    }

    fs::write(&markdown_path, md)
        .map_err(|err| format!("failed to write markdown export: {err}"))?;
    fs::write(
        &json_path,
        serde_json::to_string_pretty(&serde_json::json!({
            "project": project,
            "commands": commands,
            "sessions": sessions
        }))
        .map_err(|err| format!("failed to encode JSON export: {err}"))?,
    )
    .map_err(|err| format!("failed to write JSON export: {err}"))?;
    let db_path = database_path()?;
    if db_path.exists() {
        fs::copy(&db_path, &sqlite_path)
            .map_err(|err| format!("failed to copy SQLite backup: {err}"))?;
    }
    println!("mw: exported debug bundle {}", bundle_dir.display());
    Ok(())
}

fn global_cmd(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("on") => global_on(),
        Some("off") => global_off(),
        Some("status") | None => global_status(),
        Some(other) => Err(format!(
            "unknown subcommand {other:?}; usage: mw global [on|off|status]"
        )),
    }
}

/// Shell startup file to wire the hook into, chosen from $SHELL (zsh vs bash).
fn shell_rc_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "could not resolve home directory".to_string())?;
    let shell = env::var("SHELL").unwrap_or_default();
    let is_zsh = shell
        .rsplit('/')
        .next()
        .map_or(false, |name| name.contains("zsh"));
    Ok(home.join(if is_zsh { ".zshrc" } else { ".bashrc" }))
}

fn global_hook_path() -> Result<PathBuf, String> {
    Ok(memorywhale_dir()?.join("global-hook.sh"))
}

fn global_enabled_path() -> Result<PathBuf, String> {
    Ok(memorywhale_dir()?.join("global-enabled"))
}

const RC_MARKER: &str = "# memorywhale-global";

/// The POSIX-sh hook sourced by every interactive shell. It records only when
/// the enabled flag exists, `mw` is on PATH, the shell is interactive, and it
/// isn't already inside a recording (MW_RECORDING guard prevents any loop).
fn hook_contents(enabled_path: &str) -> String {
    format!(
        "# MemoryWhale global recording hook (managed by `mw global` — do not edit)\n\
         if [ -z \"$MW_RECORDING\" ] && [ -f \"{enabled_path}\" ] && command -v mw >/dev/null 2>&1 && case $- in *i*) true;; *) false;; esac && [ -t 0 ]; then\n\
         \x20   export MW_RECORDING=1\n\
         \x20   exec mw --notes \"auto session ($(basename \"$PWD\"))\"\n\
         fi\n"
    )
}

fn global_on() -> Result<(), String> {
    let hook_path = global_hook_path()?;
    let enabled_path = global_enabled_path()?;
    let rc_path = shell_rc_path()?;

    if let Some(parent) = hook_path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("failed to create data dir: {err}"))?;
    }
    let hook_str = hook_path
        .to_str()
        .ok_or_else(|| "hook path is not valid UTF-8".to_string())?;
    let enabled_str = enabled_path
        .to_str()
        .ok_or_else(|| "enabled-flag path is not valid UTF-8".to_string())?;

    fs::write(&hook_path, hook_contents(enabled_str))
        .map_err(|err| format!("failed to write hook: {err}"))?;

    let existing = fs::read_to_string(&rc_path).unwrap_or_default();
    let already_wired = existing.contains(RC_MARKER);
    if !already_wired {
        use std::io::Write;
        let line = format!("\n[ -f \"{hook_str}\" ] && . \"{hook_str}\"  {RC_MARKER}\n");
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&rc_path)
            .map_err(|err| format!("failed to open {}: {err}", rc_path.display()))?;
        file.write_all(line.as_bytes())
            .map_err(|err| format!("failed to update {}: {err}", rc_path.display()))?;
    }

    fs::write(&enabled_path, "enabled\n")
        .map_err(|err| format!("failed to write enabled flag: {err}"))?;

    println!("mw: global recording ENABLED.");
    if !already_wired {
        println!("  wired into: {}", rc_path.display());
    } else {
        println!("  already wired into: {} (re-enabled)", rc_path.display());
    }
    println!("  hook: {hook_str}");
    println!(
        "  Open a NEW terminal (or run `source {}`) to start auto-recording.",
        rc_path.display()
    );
    Ok(())
}

fn global_off() -> Result<(), String> {
    let enabled_path = global_enabled_path()?;
    if enabled_path.exists() {
        fs::remove_file(&enabled_path)
            .map_err(|err| format!("failed to remove enabled flag: {err}"))?;
    }
    println!("mw: global recording DISABLED. New terminals will not auto-record.");
    println!("  (Any already-open recording sessions continue until you exit.)");
    println!("  Re-enable anytime with: mw global on");
    Ok(())
}

fn global_status() -> Result<(), String> {
    let enabled = global_enabled_path()?.exists();
    let hook_path = global_hook_path()?;
    let rc_path = shell_rc_path()?;
    let wired = fs::read_to_string(&rc_path)
        .unwrap_or_default()
        .contains(RC_MARKER);

    println!("global recording: {}", if enabled { "ON" } else { "OFF" });
    println!(
        "wired into {}: {}",
        rc_path.display(),
        if wired { "yes" } else { "no" }
    );
    println!(
        "hook file: {}",
        if hook_path.exists() {
            hook_path.display().to_string()
        } else {
            "(not installed yet)".to_string()
        }
    );
    if !wired {
        println!("run `mw global on` to set it up.");
    }
    Ok(())
}

fn show_session(args: &[String]) -> Result<(), String> {
    let id: i64 = match args.first() {
        Some(value) => value
            .parse()
            .map_err(|_| format!("invalid session id {value:?}; usage: mw show <id>"))?,
        None => return Err("usage: mw show <id>".to_string()),
    };

    let conn =
        Connection::open(database_path()?).map_err(|err| format!("failed to open db: {err}"))?;
    init_schema(&conn)?;

    let row = conn.query_row(
        "SELECT started_at, cwd, notes, transcript FROM sessions WHERE id = ?1",
        params![id],
        |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, String>(3)?,
            ))
        },
    );

    match row {
        Ok((started_at, cwd, notes, transcript)) => {
            println!("=== session #{id} ===");
            println!("started: {started_at}");
            if let Some(cwd) = cwd {
                println!("cwd:     {cwd}");
            }
            if !notes.is_empty() {
                println!("notes:   {notes}");
            }
            println!("----------------------------------------");
            print!("{transcript}");
            if !transcript.ends_with('\n') {
                println!();
            }
            Ok(())
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => Err(format!(
            "no session #{id}; run `mw list` to see recorded sessions"
        )),
        Err(err) => Err(format!("failed to read session: {err}")),
    }
}

fn list_sessions() -> Result<(), String> {
    let conn =
        Connection::open(database_path()?).map_err(|err| format!("failed to open db: {err}"))?;
    init_schema(&conn)?;

    let mut stmt = conn
        .prepare("SELECT id, started_at, byte_count, notes FROM sessions ORDER BY id")
        .map_err(|err| format!("failed to query sessions: {err}"))?;
    let rows = stmt
        .query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, String>(3)?,
            ))
        })
        .map_err(|err| format!("failed to read sessions: {err}"))?;

    let mut count = 0;
    for row in rows {
        let (id, started_at, byte_count, notes) = row.map_err(|err| format!("row error: {err}"))?;
        println!("#{id}\t{started_at}\t{byte_count} bytes\t{notes}");
        count += 1;
    }
    if count == 0 {
        println!("no sessions recorded yet; run `mw` to record one");
    }
    Ok(())
}

/// Strip terminal escape sequences and control characters so the stored
/// transcript is searchable plain text. The raw file is kept on disk untouched.
fn clean_transcript(input: &str) -> String {
    // OSC sequences: ESC ] ... BEL  (or ESC \)
    let osc = Regex::new(r"\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)").unwrap();
    // CSI / other ESC-introduced sequences.
    let csi = Regex::new(r"\x1b[@-Z\\-_]|\x1b\[[0-?]*[ -/]*[@-~]").unwrap();
    // Carriage returns (script logs are full of them) and stray control chars.
    let ctrl = Regex::new(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]").unwrap();

    let s = osc.replace_all(input, "");
    let s = csi.replace_all(&s, "");
    let s = s.replace('\r', "");
    let cleaned = ctrl.replace_all(&s, "").into_owned();
    // Scrub secrets before the transcript is stored (env dumps, pasted tokens).
    mw_cli::redact(&cleaned)
}

/// Print a compact, token-budgeted digest of recent memory for an AI agent to
/// read: recent failed commands (with short error tails) and recent sessions.
fn context_cmd(args: &[String]) -> Result<(), String> {
    let mut project: Option<String> = None;
    let mut last_error = false;
    let mut limit: i64 = 8;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--last-error" => last_error = true,
            "--limit" => {
                limit = iter
                    .next()
                    .and_then(|v| v.parse().ok())
                    .ok_or_else(|| "--limit requires a number".to_string())?;
            }
            other if other.starts_with("project:") => project = Some(other.to_string()),
            other => return Err(format!("unexpected argument {other:?}; run mw --help")),
        }
    }
    let like = project.as_ref().map(|p| format!("%{p}%"));
    let conn = open_session_db()?;

    // One-line tail of the most useful error text, length-capped for token budget.
    let tail = |text: &str, max: usize| -> String {
        let t = text.trim();
        let t = t
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or(t)
            .trim();
        let chars: Vec<char> = t.chars().collect();
        if chars.len() > max {
            format!("…{}", chars[chars.len() - max..].iter().collect::<String>())
        } else {
            t.to_string()
        }
    };

    if let Some(p) = &project {
        println!("# MemoryWhale context ({p})\n");
    } else {
        println!("# MemoryWhale context\n");
    }

    // Failed commands are the highest-signal thing for a debugging agent.
    let mut stmt = conn
        .prepare(
            "SELECT argv_json, cwd, exit_code, stderr, notes, created_at
             FROM command_runs
             WHERE (exit_code IS NOT NULL AND exit_code != 0)
               AND (?1 IS NULL OR notes LIKE ?1)
             ORDER BY id DESC LIMIT ?2",
        )
        .map_err(|err| format!("failed to prepare context query: {err}"))?;
    let rows = stmt
        .query_map(params![like.as_deref(), if last_error { 1 } else { limit }], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, Option<String>>(1)?,
                r.get::<_, Option<i64>>(2)?,
                r.get::<_, String>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, String>(5)?,
            ))
        })
        .map_err(|err| format!("failed to read command runs: {err}"))?;

    let mut any = false;
    println!("## Recent failed commands");
    for row in rows {
        let (argv_json, cwd, exit_code, stderr, notes, created_at) =
            row.map_err(|err| format!("row error: {err}"))?;
        let argv: Vec<String> = serde_json::from_str(&argv_json).unwrap_or_default();
        any = true;
        println!(
            "- `{}` (exit {}, {})\n  cwd: {}\n  err: {}{}",
            argv.join(" "),
            exit_code.unwrap_or(-1),
            created_at,
            cwd.unwrap_or_default(),
            tail(&stderr, 200),
            if notes.trim().is_empty() {
                String::new()
            } else {
                format!("\n  note: {}", tail(&notes, 160))
            }
        );
    }
    if !any {
        println!("(none)");
    }

    if last_error {
        return Ok(());
    }

    // A few recent sessions, for the "what was I doing" picture.
    let mut stmt = conn
        .prepare(
            "SELECT id, notes, started_at, byte_count
             FROM sessions
             WHERE ?1 IS NULL OR notes LIKE ?1
             ORDER BY id DESC LIMIT ?2",
        )
        .map_err(|err| format!("failed to prepare session query: {err}"))?;
    let rows = stmt
        .query_map(params![like.as_deref(), limit], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .map_err(|err| format!("failed to read sessions: {err}"))?;
    println!("\n## Recent sessions");
    let mut any = false;
    for row in rows {
        let (id, notes, started_at, byte_count) = row.map_err(|err| format!("row error: {err}"))?;
        any = true;
        println!(
            "- #{id} {started_at} ({byte_count} bytes){}  — replay with `mw show {id}`",
            if notes.trim().is_empty() {
                String::new()
            } else {
                format!(": {}", tail(&notes, 160))
            }
        );
    }
    if !any {
        println!("(none)");
    }
    Ok(())
}

/// Self-check the install so a confused user (or agent) can see what's wrong.
fn doctor() -> Result<(), String> {
    let ok = |label: &str, detail: String| println!("  ok   {label}: {detail}");
    let warn = |label: &str, detail: String| println!("  WARN {label}: {detail}");

    println!("MemoryWhale doctor\n");

    // Data dir writable?
    match memorywhale_dir() {
        Ok(dir) => {
            let writable = fs::create_dir_all(&dir).is_ok()
                && {
                    let probe = dir.join(".doctor-write-test");
                    let r = fs::write(&probe, b"ok").is_ok();
                    let _ = fs::remove_file(&probe);
                    r
                };
            if writable {
                ok("data dir", dir.display().to_string());
            } else {
                warn("data dir", format!("{} (not writable)", dir.display()));
            }
        }
        Err(err) => warn("data dir", err),
    }

    // Database opens + row counts.
    match open_session_db() {
        Ok(conn) => {
            let count = |table: &str| -> i64 {
                conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                    .unwrap_or(-1)
            };
            ok(
                "database",
                format!(
                    "{} sessions, {} command runs, {} bookmarks",
                    count("sessions"),
                    count("command_runs"),
                    count("bookmarks")
                ),
            );
        }
        Err(err) => warn("database", err),
    }

    // `script` is required for `mw` session recording.
    match Command::new("script").arg("--version").output() {
        Ok(_) => ok("recording", "`script` is available".to_string()),
        Err(_) => warn(
            "recording",
            "`script` not found — session recording needs util-linux/bsdutils `script`".to_string(),
        ),
    }

    // Global hook status.
    let enabled = global_enabled_path().map(|p| p.exists()).unwrap_or(false);
    let wired = shell_rc_path()
        .ok()
        .and_then(|rc| fs::read_to_string(rc).ok())
        .map(|c| c.contains(RC_MARKER))
        .unwrap_or(false);
    if enabled && wired {
        ok("auto-record", "on and wired into your shell".to_string());
    } else {
        warn(
            "auto-record",
            format!(
                "off (enabled: {enabled}, wired: {wired}) — run `mw global on` to enable"
            ),
        );
    }

    Ok(())
}

fn init_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;

        CREATE TABLE IF NOT EXISTS sessions (
            id INTEGER PRIMARY KEY,
            shell TEXT,
            cwd TEXT,
            transcript_path TEXT NOT NULL,
            transcript TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '',
            started_at TEXT NOT NULL,
            ended_at TEXT NOT NULL,
            byte_count INTEGER NOT NULL DEFAULT 0,
            status TEXT NOT NULL DEFAULT 'finished'
        );

        CREATE INDEX IF NOT EXISTS idx_sessions_started_at ON sessions(started_at);

        CREATE TABLE IF NOT EXISTS command_runs (
            id INTEGER PRIMARY KEY,
            command TEXT NOT NULL,
            argv_json TEXT NOT NULL,
            cwd TEXT,
            exit_code INTEGER,
            stdout TEXT NOT NULL DEFAULT '',
            stderr TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '',
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS command_arguments (
            id INTEGER PRIMARY KEY,
            command_run_id INTEGER NOT NULL,
            position INTEGER NOT NULL,
            value TEXT NOT NULL,
            FOREIGN KEY(command_run_id) REFERENCES command_runs(id) ON DELETE CASCADE
        );

        CREATE TABLE IF NOT EXISTS bookmarks (
            id INTEGER PRIMARY KEY,
            label TEXT NOT NULL,
            cwd TEXT,
            created_at TEXT NOT NULL,
            command_run_id INTEGER,
            session_id INTEGER
        );

        CREATE INDEX IF NOT EXISTS idx_command_runs_command ON command_runs(command);
        CREATE INDEX IF NOT EXISTS idx_command_runs_exit_code ON command_runs(exit_code);
        CREATE INDEX IF NOT EXISTS idx_command_arguments_value ON command_arguments(value);
        CREATE INDEX IF NOT EXISTS idx_bookmarks_created_at ON bookmarks(created_at);
        ",
    )
    .map_err(|err| format!("failed to initialize schema: {err}"))?;
    let _ = conn.execute(
        "ALTER TABLE sessions ADD COLUMN status TEXT NOT NULL DEFAULT 'finished'",
        [],
    );
    Ok(())
}

fn append_environment_tags(notes: String) -> String {
    let mut tags = Vec::new();
    tags.push(format!("os:{}", env::consts::OS));
    if PathBuf::from("/.dockerenv").exists() || env::var_os("container").is_some() {
        tags.push("runtime:container".to_string());
    } else {
        tags.push("runtime:host".to_string());
    }
    if env::var_os("SSH_CONNECTION").is_some() || env::var_os("SSH_CLIENT").is_some() {
        tags.push("session:ssh".to_string());
    }
    if PathBuf::from("/etc/nv_tegra_release").exists() {
        tags.push("host:jetson".to_string());
    }

    if notes.trim().is_empty() {
        tags.join(" ")
    } else {
        format!("{} {}", notes.trim(), tags.join(" "))
    }
}

fn memorywhale_dir() -> Result<PathBuf, String> {
    if let Some(path) = env::var_os("MEMORYWHALE_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }

    let base = dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "could not resolve local data directory".to_string())?;
    Ok(base.join("MemoryWhale"))
}

fn sessions_dir() -> Result<PathBuf, String> {
    Ok(memorywhale_dir()?.join("sessions"))
}

fn database_path() -> Result<PathBuf, String> {
    Ok(memorywhale_dir()?.join("memorywhale.sqlite3"))
}

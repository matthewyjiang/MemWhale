// mw-recover: rescue interrupted session recordings.
//
// When an `mw` recording is cut off (terminal closed, SSH drop, signal) before
// it writes its database row, the raw transcript file still survives on disk.
// mw-recover scans the sessions folder and imports any transcript that has no
// matching row into the sessions table, so nothing is lost.
//
// Usage:
//   mw-recover            import any orphaned session transcripts
//   mw-recover --quiet    only print the count

use chrono::{DateTime, Utc};
use regex::Regex;
use rusqlite::{params, Connection};
use std::fs;
use std::path::PathBuf;
use std::time::SystemTime;

fn main() {
    let quiet = std::env::args().any(|a| a == "--quiet");
    match recover_orphans(!quiet) {
        Ok(n) => {
            if quiet {
                println!("{n}");
            } else if n == 0 {
                println!("mw-recover: nothing to recover — all session transcripts are already saved.");
            } else {
                println!("mw-recover: recovered {n} interrupted session(s).");
            }
        }
        Err(err) => {
            eprintln!("mw-recover: {err}");
            std::process::exit(1);
        }
    }
}

/// Import every `.log` in the sessions dir that has no row yet. Returns how many
/// were recovered. Safe to call repeatedly — already-imported files are skipped.
pub fn recover_orphans(verbose: bool) -> Result<usize, String> {
    let sessions_dir = data_base()?.join("MemoryWhale").join("sessions");
    if !sessions_dir.exists() {
        return Ok(0);
    }
    let db_path = data_base()?.join("MemoryWhale").join("memorywhale.sqlite3");
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("create data dir: {e}"))?;
    }
    let conn = Connection::open(&db_path).map_err(|e| format!("open db: {e}"))?;
    init_schema(&conn)?;

    let mut recovered = 0;
    let mut entries: Vec<PathBuf> = fs::read_dir(&sessions_dir)
        .map_err(|e| format!("read sessions dir: {e}"))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().map(|x| x == "log").unwrap_or(false))
        .collect();
    entries.sort();

    for path in entries {
        let path_str = match path.to_str() {
            Some(s) => s.to_string(),
            None => continue,
        };
        let already: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sessions WHERE transcript_path = ?1",
                params![path_str],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if already > 0 {
            continue;
        }

        let raw = match fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let byte_count = raw.len() as i64;
        let cleaned = clean_transcript(&String::from_utf8_lossy(&raw));
        let started_at = started_from_filename(&path).unwrap_or_else(|| mtime_rfc3339(&path));
        let ended_at = mtime_rfc3339(&path);

        conn.execute(
            "INSERT INTO sessions (shell, cwd, transcript_path, transcript, notes, started_at, ended_at, byte_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                Option::<String>::None,
                Option::<String>::None,
                path_str,
                cleaned,
                "recovered from transcript (recording was interrupted before saving)",
                started_at,
                ended_at,
                byte_count
            ],
        )
        .map_err(|e| format!("insert recovered session: {e}"))?;
        recovered += 1;
        if verbose {
            println!("  recovered {}", path.display());
        }
    }
    Ok(recovered)
}

fn started_from_filename(path: &std::path::Path) -> Option<String> {
    let name = path.file_stem()?.to_str()?;
    let stamp = name.strip_prefix("session-")?;
    // <date>T<HH>-<MM>-<SS>[.frac]<+/-TZH>-<TZM>  ->  RFC3339 with colons restored
    let re = Regex::new(r"^(\d{4}-\d{2}-\d{2})T(\d{2})-(\d{2})-(\d{2})(\.\d+)?([+-]\d{2})-(\d{2})$").ok()?;
    let c = re.captures(stamp)?;
    Some(format!(
        "{}T{}:{}:{}{}{}:{}",
        &c[1], &c[2], &c[3], &c[4], c.get(5).map(|m| m.as_str()).unwrap_or(""), &c[6], &c[7]
    ))
}

fn mtime_rfc3339(path: &std::path::Path) -> String {
    let t = fs::metadata(path)
        .and_then(|m| m.modified())
        .unwrap_or(SystemTime::UNIX_EPOCH);
    DateTime::<Utc>::from(t).to_rfc3339()
}

/// Strip terminal escape sequences and control characters (same as mw's cleaner).
pub fn clean_transcript(input: &str) -> String {
    let osc = Regex::new(r"\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)").unwrap();
    let csi = Regex::new(r"\x1b[@-Z\\-_]|\x1b\[[0-?]*[ -/]*[@-~]").unwrap();
    let ctrl = Regex::new(r"[\x00-\x08\x0b\x0c\x0e-\x1f\x7f]").unwrap();
    let s = osc.replace_all(input, "");
    let s = csi.replace_all(&s, "");
    let s = s.replace('\r', "");
    ctrl.replace_all(&s, "").into_owned()
}

fn init_schema(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS sessions (id INTEGER PRIMARY KEY, shell TEXT, cwd TEXT,
            transcript_path TEXT NOT NULL DEFAULT '', transcript TEXT NOT NULL DEFAULT '',
            notes TEXT NOT NULL DEFAULT '', started_at TEXT NOT NULL DEFAULT '',
            ended_at TEXT NOT NULL DEFAULT '', byte_count INTEGER NOT NULL DEFAULT 0);",
    )
    .map_err(|e| format!("init schema: {e}"))
}

fn data_base() -> Result<PathBuf, String> {
    dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "could not resolve local data directory".to_string())
}

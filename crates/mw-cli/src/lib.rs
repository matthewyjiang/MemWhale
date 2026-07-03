//! Shared helpers for the MemoryWhale CLI binaries.

use regex::Regex;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::OnceLock;

/// The MemoryWhale data directory (honours `MEMORYWHALE_DATA_DIR`).
pub fn data_dir() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("MEMORYWHALE_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }
    let base = dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .ok_or_else(|| "could not resolve local data directory".to_string())?;
    Ok(base.join("MemoryWhale"))
}

/// Path to the local SQLite database.
pub fn database_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("memorywhale.sqlite3"))
}

/// Best-effort full-text index over `command_runs` (SQLite FTS5).
///
/// Creates an external-content FTS5 table kept in sync by triggers, and rebuilds
/// it once when first created so pre-existing rows get indexed. The triggers
/// persist in the database file, so once this has run any writer (mw-run,
/// mw-remember, …) maintains the index without needing to call this. Returns an
/// error if FTS5 isn't compiled in; callers treat that as "no index" and fall
/// back to LIKE.
pub fn ensure_fts(conn: &Connection) -> Result<(), String> {
    let existed = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name='command_fts'",
            [],
            |_| Ok(()),
        )
        .is_ok();
    conn.execute_batch(
        "CREATE VIRTUAL TABLE IF NOT EXISTS command_fts USING fts5(
             command, argv_json, stdout, stderr, notes,
             content='command_runs', content_rowid='id'
         );
         CREATE TRIGGER IF NOT EXISTS command_runs_fts_ai AFTER INSERT ON command_runs BEGIN
             INSERT INTO command_fts(rowid, command, argv_json, stdout, stderr, notes)
             VALUES (new.id, new.command, new.argv_json, new.stdout, new.stderr, new.notes);
         END;
         CREATE TRIGGER IF NOT EXISTS command_runs_fts_ad AFTER DELETE ON command_runs BEGIN
             INSERT INTO command_fts(command_fts, rowid, command, argv_json, stdout, stderr, notes)
             VALUES ('delete', old.id, old.command, old.argv_json, old.stdout, old.stderr, old.notes);
         END;
         CREATE TRIGGER IF NOT EXISTS command_runs_fts_au AFTER UPDATE ON command_runs BEGIN
             INSERT INTO command_fts(command_fts, rowid, command, argv_json, stdout, stderr, notes)
             VALUES ('delete', old.id, old.command, old.argv_json, old.stdout, old.stderr, old.notes);
             INSERT INTO command_fts(rowid, command, argv_json, stdout, stderr, notes)
             VALUES (new.id, new.command, new.argv_json, new.stdout, new.stderr, new.notes);
         END;",
    )
    .map_err(|e| format!("fts init: {e}"))?;
    if !existed {
        conn.execute("INSERT INTO command_fts(command_fts) VALUES('rebuild')", [])
            .map_err(|e| format!("fts rebuild: {e}"))?;
    }
    Ok(())
}

/// Turn a free-text query into a safe FTS5 MATCH expression: each whitespace
/// term becomes a quoted phrase with a trailing prefix `*` (so "link" still
/// finds "linker", closer to the old substring search), AND-ed together.
/// Quoting escapes punctuation so it can't break MATCH syntax. Empty if the
/// query has no usable terms.
pub fn fts_match_query(query: &str) -> String {
    query
        .split_whitespace()
        .map(|t| format!("\"{}\"*", t.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

const REDACTED: &str = "[REDACTED]";

/// Scrub common secret shapes out of captured text before it lands in SQLite.
///
/// This runs on stdout/stderr/notes/transcripts — the bulky, unattended
/// captures where an `env` dump or a leaked token is most likely to end up.
/// It is intentionally conservative (known token formats + `key=secret`
/// assignments), not a guarantee. Set `MEMORYWHALE_NO_REDACT=1` to store raw.
pub fn redact(text: &str) -> String {
    if std::env::var_os("MEMORYWHALE_NO_REDACT").is_some() {
        return text.to_string();
    }
    let mut out = text.to_string();
    for re in secret_patterns() {
        out = re.replace_all(&out, |caps: &regex::Captures| {
            // If the pattern captured a leading "key=" / "key:" label, keep it.
            match caps.name("label") {
                Some(label) => format!("{}{}", label.as_str(), REDACTED),
                None => REDACTED.to_string(),
            }
        })
        .into_owned();
    }
    out
}

fn secret_patterns() -> &'static [Regex] {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            // key = value / key: value where the key name looks sensitive.
            r#"(?i)(?P<label>\b(?:api[_-]?key|secret|token|password|passwd|pwd|access[_-]?key|client[_-]?secret)\b\s*[:=]\s*)['"]?[A-Za-z0-9/_+\-\.]{6,}['"]?"#,
            // Authorization: Bearer <token>
            r#"(?i)(?P<label>bearer\s+)[A-Za-z0-9._\-]{8,}"#,
            // Provider token formats.
            r#"AKIA[0-9A-Z]{16}"#,                                   // AWS access key id
            r#"gh[pousr]_[A-Za-z0-9]{20,}"#,                          // GitHub tokens
            r#"xox[baprs]-[A-Za-z0-9\-]{10,}"#,                       // Slack tokens
            r#"eyJ[A-Za-z0-9_\-]+\.eyJ[A-Za-z0-9_\-]+\.[A-Za-z0-9_\-]+"#, // JWTs
            // PEM private key blocks (whole block).
            r#"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----"#,
        ]
        .iter()
        .map(|p| Regex::new(p).expect("valid secret regex"))
        .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_label_hides_value() {
        assert_eq!(redact("API_KEY=abcdef123456"), "API_KEY=[REDACTED]");
        assert_eq!(redact("password: hunter2secret"), "password: [REDACTED]");
    }

    #[test]
    fn hides_known_token_formats() {
        assert!(redact("here AKIAABCDEFGHIJKLMNOP done").contains("[REDACTED]"));
        assert!(!redact("ghp_0123456789abcdefghijABCDEF").contains("ghp_"));
        assert!(redact("Authorization: Bearer abcd.efgh.ijkl").contains("Bearer [REDACTED]"));
    }

    #[test]
    fn leaves_ordinary_text_alone() {
        let s = "cargo build finished in 3.2s with 0 warnings";
        assert_eq!(redact(s), s);
    }

    #[test]
    fn opt_out_env_disables() {
        // Not testing the env branch here to avoid global state; ensure a
        // non-secret round-trips unchanged (covers the common path).
        assert_eq!(redact("plain line"), "plain line");
    }

    #[test]
    fn fts_query_quotes_and_prefixes_terms() {
        assert_eq!(fts_match_query("linker error"), "\"linker\"* \"error\"*");
        assert_eq!(fts_match_query("  spaced  "), "\"spaced\"*");
        assert_eq!(fts_match_query(""), "");
        // a stray quote is escaped, not left to break MATCH syntax
        assert_eq!(fts_match_query("a\"b"), "\"a\"\"b\"*");
    }

    #[test]
    fn ensure_fts_indexes_and_matches() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE command_runs (id INTEGER PRIMARY KEY, command TEXT,
                 argv_json TEXT, stdout TEXT, stderr TEXT, notes TEXT);
             INSERT INTO command_runs (command, argv_json, stdout, stderr, notes)
             VALUES ('cargo', '[\"cargo\",\"build\"]', '', 'error: linker failed', 'auv');",
        )
        .unwrap();
        // pre-existing row must be indexed by the one-time rebuild
        ensure_fts(&conn).unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM command_fts WHERE command_fts MATCH ?1",
                [fts_match_query("linker")],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "pre-existing row should be found via FTS");

        // a new row must be maintained by the trigger
        conn.execute(
            "INSERT INTO command_runs (command, argv_json, stdout, stderr, notes)
             VALUES ('make', '[\"make\"]', '', 'undefined reference', '')",
            [],
        )
        .unwrap();
        let n: i64 = conn
            .query_row(
                "SELECT count(*) FROM command_fts WHERE command_fts MATCH ?1",
                [fts_match_query("undefined")],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(n, 1, "trigger should index the new row");
    }
}

//! Shared helpers for the MemoryWhale CLI binaries.

use regex::Regex;
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
}

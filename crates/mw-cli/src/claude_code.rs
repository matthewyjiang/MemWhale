//! Claude Code integration: capture hook, skill, and MCP registration.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

// In-crate so they ship inside the published package (same pattern as shell hooks).
const HOOK_SCRIPT: &str = include_str!("../claude-code/mw-record.py");
const SKILL: &str = include_str!("../claude-code/SKILL.md");

const MCP_SCOPE: &[&str] = &["--scope", "user"];

/// Paths written or updated by [`install`].
pub struct InstallResult {
    pub config_dir: PathBuf,
    pub hook_path: PathBuf,
    pub settings_path: PathBuf,
    pub skill_path: PathBuf,
    pub mcp_registered: bool,
}

/// What [`revert`] removed or updated.
pub struct RevertResult {
    pub config_dir: PathBuf,
    pub hook_removed: bool,
    pub skill_removed: bool,
    pub settings_updated: bool,
    pub mcp_unregistered: bool,
}

/// Install MemoryWhale into Claude Code: capture hook, skill, and (when
/// possible) the user-scoped `memorywhale` MCP server.
pub fn install() -> Result<InstallResult, String> {
    let config_dir = claude_config_dir()?;
    let hook_path = config_dir.join("hooks/mw-record.py");
    let skill_path = config_dir.join("skills/memorywhale/SKILL.md");
    let settings_path = config_dir.join("settings.json");

    let existing = read_settings(&settings_path)?;
    let (updated, settings_changed) = merge_settings(&existing, &hook_path)?;

    fs::create_dir_all(config_dir.join("hooks"))
        .map_err(|err| format!("failed to create {}: {err}", config_dir.join("hooks").display()))?;
    fs::create_dir_all(config_dir.join("skills").join("memorywhale")).map_err(|err| {
        format!(
            "failed to create {}: {err}",
            config_dir.join("skills/memorywhale").display()
        )
    })?;

    fs::write(&hook_path, HOOK_SCRIPT)
        .map_err(|err| format!("failed to write {}: {err}", hook_path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook_path, fs::Permissions::from_mode(0o755))
            .map_err(|err| format!("failed to chmod {}: {err}", hook_path.display()))?;
    }

    fs::write(&skill_path, SKILL)
        .map_err(|err| format!("failed to write {}: {err}", skill_path.display()))?;

    if settings_changed {
        atomic_write(&settings_path, &updated)?;
    }

    let mcp_registered = register_mcp();

    Ok(InstallResult {
        config_dir,
        hook_path,
        settings_path,
        skill_path,
        mcp_registered,
    })
}

/// Undo [`install`]: drop our settings entry, then remove the hook and skill, and
/// unregister the user-scoped MCP server when the Claude Code CLI is available.
pub fn revert() -> Result<RevertResult, String> {
    let config_dir = claude_config_dir()?;
    let hook_path = config_dir.join("hooks/mw-record.py");
    let skill_path = config_dir.join("skills/memorywhale/SKILL.md");
    let skill_dir = config_dir.join("skills/memorywhale");
    let settings_path = config_dir.join("settings.json");

    let existing = read_settings(&settings_path)?;
    let (updated, settings_changed) = if settings_path.exists() {
        unmerge_settings(&existing, &hook_path)?
    } else {
        (String::new(), false)
    };

    if settings_changed {
        if updated.trim().is_empty() {
            let _ = fs::remove_file(&settings_path);
        } else {
            atomic_write(&settings_path, &updated)?;
        }
    }

    let hook_removed = if hook_path.is_file() {
        fs::remove_file(&hook_path)
            .map_err(|err| format!("failed to remove {}: {err}", hook_path.display()))?;
        true
    } else {
        false
    };

    let skill_removed = if skill_path.is_file() {
        fs::remove_file(&skill_path)
            .map_err(|err| format!("failed to remove {}: {err}", skill_path.display()))?;
        let _ = fs::remove_dir(&skill_dir);
        true
    } else {
        false
    };

    let mcp_unregistered = unregister_mcp();

    Ok(RevertResult {
        config_dir,
        hook_removed,
        skill_removed,
        settings_updated: settings_changed,
        mcp_unregistered,
    })
}

/// Claude Code config root. Override with `CLAUDE_CONFIG_DIR` (for tests).
pub fn claude_config_dir() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("CLAUDE_CONFIG_DIR") {
        return Ok(PathBuf::from(path));
    }
    dirs::home_dir()
        .ok_or_else(|| "could not resolve the home directory".to_string())
        .map(|home| home.join(".claude"))
}

fn read_settings(settings_path: &Path) -> Result<String, String> {
    if settings_path.exists() {
        fs::read_to_string(settings_path)
            .map_err(|err| format!("failed to read {}: {err}", settings_path.display()))
    } else {
        Ok(String::new())
    }
}

fn hook_command(hook_path: &Path) -> String {
    format!("python3 \"{}\"", hook_path.display())
}

/// Commands written by MemoryWhale for the Claude capture hook.
fn is_memorywhale_hook_command(command: Option<&str>) -> bool {
    let Some(command) = command else {
        return false;
    };
    command.starts_with("python3 \"") && command.ends_with("hooks/mw-record.py\"")
}

fn parse_settings(existing: &str) -> Result<Value, String> {
    if existing.trim().is_empty() {
        return Ok(json!({}));
    }
    let root: Value = serde_json::from_str(existing).map_err(|err| {
        format!("invalid Claude settings.json; file was not changed: {err}")
    })?;
    if !root.is_object() {
        return Err("invalid Claude settings.json; expected a top-level object".to_string());
    }
    Ok(root)
}

fn serialize_settings(root: &Value) -> Result<String, String> {
    if root.as_object().is_some_and(|obj| obj.is_empty()) {
        return Ok(String::new());
    }
    serde_json::to_string_pretty(root)
        .map(|s| format!("{s}\n"))
        .map_err(|err| format!("failed to serialize settings.json: {err}"))
}

fn merge_settings(existing: &str, hook_path: &Path) -> Result<(String, bool), String> {
    let before = parse_settings(existing)?;
    let mut root = before.clone();

    let command = hook_command(hook_path);
    let entry = json!({
        "type": "command",
        "command": command,
    });

    let hooks = root
        .as_object_mut()
        .expect("checked above")
        .entry("hooks")
        .or_insert_with(|| json!({}));
    if !hooks.is_object() {
        return Err(
            "invalid Claude settings.json; hooks must be an object and file was not changed"
                .to_string(),
        );
    }

    let post_tool_use = hooks
        .as_object_mut()
        .expect("checked above")
        .entry("PostToolUse")
        .or_insert_with(|| json!([]));
    if !post_tool_use.is_array() {
        return Err(
            "invalid Claude settings.json; hooks.PostToolUse must be an array and file was not changed"
                .to_string(),
        );
    }

    let groups = post_tool_use.as_array_mut().expect("checked above");
    if let Some(bash_group) = groups
        .iter_mut()
        .find(|group| group.get("matcher").and_then(Value::as_str) == Some("Bash"))
    {
        let hook_list = bash_group
            .as_object_mut()
            .and_then(|obj| obj.get_mut("hooks"))
            .ok_or_else(|| {
                "invalid Claude settings.json; Bash hook group is missing a hooks array".to_string()
            })?;
        if !hook_list.is_array() {
            return Err(
                "invalid Claude settings.json; Bash hook group hooks must be an array".to_string(),
            );
        }
        let hooks_array = hook_list.as_array_mut().expect("checked above");
        hooks_array.retain(|hook| {
            !is_memorywhale_hook_command(hook.get("command").and_then(Value::as_str))
        });
        hooks_array.push(entry);
    } else {
        groups.push(json!({
            "matcher": "Bash",
            "hooks": [entry],
        }));
    }

    if root == before {
        return Ok((existing.to_string(), false));
    }
    Ok((serialize_settings(&root)?, true))
}

fn unmerge_settings(existing: &str, hook_path: &Path) -> Result<(String, bool), String> {
    if existing.trim().is_empty() {
        return Ok((String::new(), false));
    }

    let before = parse_settings(existing)?;
    let mut root = before.clone();

    let Some(hooks) = root.get_mut("hooks") else {
        return Ok((existing.to_string(), false));
    };
    if !hooks.is_object() {
        return Err(
            "invalid Claude settings.json; hooks must be an object and file was not changed"
                .to_string(),
        );
    }

    let Some(post_tool_use) = hooks.get_mut("PostToolUse") else {
        return Ok((existing.to_string(), false));
    };
    if !post_tool_use.is_array() {
        return Err(
            "invalid Claude settings.json; hooks.PostToolUse must be an array and file was not changed"
                .to_string(),
        );
    }

    let expected = hook_command(hook_path);
    let groups = post_tool_use.as_array_mut().expect("checked above");
    groups.retain_mut(|group| {
        if group.get("matcher").and_then(Value::as_str) != Some("Bash") {
            return true;
        }
        let Some(hook_list) = group.as_object_mut().and_then(|obj| obj.get_mut("hooks")) else {
            return true;
        };
        if !hook_list.is_array() {
            return true;
        }
        let hooks_array = hook_list.as_array_mut().expect("checked above");
        hooks_array.retain(|hook| {
            hook.get("command").and_then(Value::as_str) != Some(expected.as_str())
                && !is_memorywhale_hook_command(hook.get("command").and_then(Value::as_str))
        });
        !hooks_array.is_empty()
    });

    if groups.is_empty() {
        hooks.as_object_mut().expect("checked above").remove("PostToolUse");
    }
    if hooks.as_object().is_some_and(|obj| obj.is_empty()) {
        root.as_object_mut().expect("checked above").remove("hooks");
    }

    if root == before {
        return Ok((existing.to_string(), false));
    }
    Ok((serialize_settings(&root)?, true))
}

fn atomic_write(path: &Path, contents: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("settings path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("settings path has no file name: {}", path.display()))?;
    let tmp = parent.join(format!(".{}.tmp", file_name.to_string_lossy()));
    fs::write(&tmp, contents).map_err(|err| format!("failed to write {}: {err}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|err| {
        let _ = fs::remove_file(&tmp);
        format!("failed to write {}: {err}", path.display())
    })
}

fn unregister_mcp() -> bool {
    let Some(claude) = which("claude") else {
        return false;
    };
    if !mcp_already_registered(&claude) {
        return false;
    }
    Command::new(&claude)
        .arg("mcp")
        .arg("remove")
        .args(MCP_SCOPE)
        .arg("memorywhale")
        .status()
        .ok()
        .is_some_and(|status| status.success())
}

fn register_mcp() -> bool {
    let Some(claude) = which("claude") else {
        return false;
    };
    if mcp_already_registered(&claude) {
        return true;
    }
    Command::new(&claude)
        .arg("mcp")
        .arg("add")
        .args(MCP_SCOPE)
        .arg("--transport")
        .arg("stdio")
        .arg("memorywhale")
        .arg("--")
        .arg("mw-mcp")
        .status()
        .ok()
        .is_some_and(|status| status.success())
}

fn mcp_already_registered(claude: &str) -> bool {
    Command::new(claude)
        .arg("mcp")
        .arg("get")
        .args(MCP_SCOPE)
        .arg("memorywhale")
        .output()
        .ok()
        .is_some_and(|output| output.status.success())
}

fn which(name: &str) -> Option<String> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let direct = dir.join(name);
            if direct.is_file() {
                return Some(direct.to_string_lossy().into_owned());
            }
            #[cfg(windows)]
            {
                let exe = dir.join(format!("{name}.exe"));
                if exe.is_file() {
                    return Some(exe.to_string_lossy().into_owned());
                }
            }
            None
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merge_settings_adds_bash_hook_to_empty_config() {
        let hook = PathBuf::from("/home/me/.claude/hooks/mw-record.py");
        let (merged, changed) = merge_settings("", &hook).unwrap();
        assert!(changed);
        let value: Value = serde_json::from_str(&merged).unwrap();
        let command = value["hooks"]["PostToolUse"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        assert_eq!(command, hook_command(&hook));
        assert_eq!(value["hooks"]["PostToolUse"][0]["matcher"], "Bash");
    }

    #[test]
    fn merge_settings_preserves_other_settings_and_is_idempotent() {
        let hook = PathBuf::from("/tmp/.claude/hooks/mw-record.py");
        let original = r#"{
  "theme": "dark",
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Read",
        "hooks": [{"type": "command", "command": "echo read"}]
      }
    ]
  }
}"#;
        let (once, changed_once) = merge_settings(original, &hook).unwrap();
        assert!(changed_once);
        let parsed: Value = serde_json::from_str(&once).unwrap();
        assert_eq!(parsed["theme"], "dark");
        assert_eq!(parsed["hooks"]["PostToolUse"].as_array().unwrap().len(), 2);

        let (twice, changed_twice) = merge_settings(&once, &hook).unwrap();
        assert!(!changed_twice);
        assert_eq!(twice, once);
    }

    #[test]
    fn merge_settings_updates_stale_hook_path() {
        let hook = PathBuf::from("/new/home/.claude/hooks/mw-record.py");
        let existing = format!(
            r#"{{
  "hooks": {{
    "PostToolUse": [
      {{
        "matcher": "Bash",
        "hooks": [{{"type": "command", "command": "python3 \"/old/home/.claude/hooks/mw-record.py\""}}]
      }}
    ]
  }}
}}"#
        );
        let (merged, changed) = merge_settings(&existing, &hook).unwrap();
        assert!(changed);
        let command = serde_json::from_str::<Value>(&merged).unwrap()["hooks"]["PostToolUse"][0]
            ["hooks"][0]["command"]
            .as_str()
            .unwrap()
            .to_string();
        assert_eq!(command, hook_command(&hook));
    }

    #[test]
    fn merge_settings_rejects_invalid_json() {
        let err = merge_settings("{not json", &PathBuf::from("/tmp/hook.py")).unwrap_err();
        assert!(err.contains("invalid Claude settings.json"));
    }

    #[test]
    fn unmerge_settings_removes_only_memorywhale_bash_hook() {
        let hook = PathBuf::from("/tmp/.claude/hooks/mw-record.py");
        let (installed, _) = merge_settings(
            r#"{
  "theme": "dark",
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Read",
        "hooks": [{"type": "command", "command": "echo read"}]
      },
      {
        "matcher": "Bash",
        "hooks": [
          {"type": "command", "command": "echo other"},
          {"type": "command", "command": "python3 \"/tmp/.claude/hooks/mw-record.py\""}
        ]
      }
    ]
  }
}"#,
            &hook,
        )
        .unwrap();
        let (reverted, changed) = unmerge_settings(&installed, &hook).unwrap();
        assert!(changed);
        let parsed: Value = serde_json::from_str(&reverted).unwrap();
        assert_eq!(parsed["theme"], "dark");
        let bash_hooks = parsed["hooks"]["PostToolUse"]
            .as_array()
            .unwrap()
            .iter()
            .find(|group| group.get("matcher") == Some(&json!("Bash")))
            .unwrap()["hooks"]
            .as_array()
            .unwrap();
        assert_eq!(bash_hooks.len(), 1);
        assert_eq!(bash_hooks[0]["command"], "echo other");
    }

    #[test]
    fn unmerge_settings_drops_empty_hook_groups() {
        let hook = PathBuf::from("/tmp/.claude/hooks/mw-record.py");
        let (installed, _) = merge_settings("", &hook).unwrap();
        let (reverted, changed) = unmerge_settings(&installed, &hook).unwrap();
        assert!(changed);
        assert!(reverted.trim().is_empty());
    }

    #[test]
    fn unmerge_settings_is_unchanged_without_memorywhale_hook() {
        let hook = PathBuf::from("/tmp/.claude/hooks/mw-record.py");
        let original = r#"{"theme":"dark"}"#;
        let (updated, changed) = unmerge_settings(original, &hook).unwrap();
        assert!(!changed);
        assert_eq!(updated, original);
    }

    #[test]
    fn unmerge_settings_does_not_remove_unrelated_bash_hooks() {
        let hook = PathBuf::from("/tmp/.claude/hooks/mw-record.py");
        let original = r#"{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{"type": "command", "command": "echo mentions mw-record.py in text"}]
      }
    ]
  }
}"#;
        let (updated, changed) = unmerge_settings(original, &hook).unwrap();
        assert!(!changed);
        assert_eq!(updated, original);
    }

    #[test]
    fn bundled_assets_match_integrations_tree() {
        let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let repo = manifest.join("../..");
        let pairs = [
            (
                manifest.join("claude-code/mw-record.py"),
                repo.join("integrations/claude-code/hooks/mw-record.py"),
            ),
            (
                manifest.join("claude-code/SKILL.md"),
                repo.join("integrations/claude-code/memorywhale/SKILL.md"),
            ),
        ];
        for (crate_copy, integrations_copy) in pairs {
            let crate_bytes = fs::read(&crate_copy).unwrap_or_else(|err| {
                panic!("failed to read {}: {err}", crate_copy.display())
            });
            let integrations_bytes = fs::read(&integrations_copy).unwrap_or_else(|err| {
                panic!("failed to read {}: {err}", integrations_copy.display())
            });
            assert_eq!(
                crate_bytes, integrations_bytes,
                "{} and {} drifted — update both copies",
                crate_copy.display(),
                integrations_copy.display()
            );
        }
    }
}

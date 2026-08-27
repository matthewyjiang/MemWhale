//! Claude Code integration: capture hook, skill, and MCP registration.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

// In-crate so they ship inside the published package (same pattern as shell hooks).
const HOOK_SCRIPT: &str = include_str!("../claude-code/mw-record.py");
const SKILL: &str = include_str!("../claude-code/SKILL.md");

const MCP_ADD: &str = "claude mcp add --scope user --transport stdio memorywhale -- mw-mcp";
const MCP_REMOVE: &str = "claude mcp remove --scope user memorywhale";

/// `mw integrate claude [--revert]`
pub fn cli(args: &[String]) -> Result<(), String> {
    let mut revert = false;
    for arg in args {
        match arg.as_str() {
            "--revert" => revert = true,
            _ => return Err("usage: mw integrate claude [--revert]".to_string()),
        }
    }
    if revert {
        report_revert(uninstall()?);
    } else {
        report_install(install()?);
    }
    Ok(())
}

struct InstallResult {
    config_dir: PathBuf,
    hook_path: PathBuf,
    settings_path: PathBuf,
    skill_path: PathBuf,
    mcp: McpOutcome,
}

struct RevertResult {
    config_dir: PathBuf,
    hook_removed: bool,
    skill_removed: bool,
    settings_updated: bool,
    mcp: McpOutcome,
}

enum McpOutcome {
    /// `claude mcp add`/`remove` succeeded.
    Changed,
    /// Already in the desired state.
    Unchanged,
    /// `claude` is not on PATH.
    CliMissing,
    /// `claude` ran but the add/remove failed.
    Failed,
}

struct ClaudePaths {
    config_dir: PathBuf,
    hook_path: PathBuf,
    skill_path: PathBuf,
    skill_dir: PathBuf,
    settings_path: PathBuf,
}

impl ClaudePaths {
    fn resolve() -> Result<Self, String> {
        let config_dir = claude_config_dir()?;
        let skill_dir = config_dir.join("skills/memorywhale");
        Ok(Self {
            hook_path: config_dir.join("hooks/mw-record.py"),
            skill_path: skill_dir.join("SKILL.md"),
            settings_path: config_dir.join("settings.json"),
            skill_dir,
            config_dir,
        })
    }
}

#[derive(Clone, Default, Deserialize, PartialEq, Serialize)]
struct ClaudeSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hooks: Option<Hooks>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Clone, Default, Deserialize, PartialEq, Serialize)]
struct Hooks {
    #[serde(rename = "PostToolUse", default, skip_serializing_if = "Vec::is_empty")]
    post_tool_use: Vec<HookGroup>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

impl Hooks {
    fn is_empty(&self) -> bool {
        self.post_tool_use.is_empty() && self.extra.is_empty()
    }
}

#[derive(Clone, Deserialize, PartialEq, Serialize)]
struct HookGroup {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    matcher: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    hooks: Option<Vec<HookEntry>>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

#[derive(Clone, Deserialize, PartialEq, Serialize)]
struct HookEntry {
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    hook_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    command: Option<String>,
    #[serde(flatten)]
    extra: Map<String, Value>,
}

impl HookEntry {
    fn command(hook_path: &Path) -> Self {
        Self {
            hook_type: Some("command".to_string()),
            command: Some(hook_command(hook_path)),
            extra: Map::new(),
        }
    }

    fn is_memorywhale(&self) -> bool {
        self.command
            .as_deref()
            .is_some_and(is_memorywhale_hook_command)
    }
}

fn install() -> Result<InstallResult, String> {
    let paths = ClaudePaths::resolve()?;
    let existing = read_settings(&paths.settings_path)?;
    let (updated, settings_changed) = merge_settings(&existing, &paths.hook_path)?;

    let hooks_dir = paths.config_dir.join("hooks");
    fs::create_dir_all(&hooks_dir)
        .map_err(|err| format!("failed to create {}: {err}", hooks_dir.display()))?;
    fs::create_dir_all(&paths.skill_dir)
        .map_err(|err| format!("failed to create {}: {err}", paths.skill_dir.display()))?;

    fs::write(&paths.hook_path, HOOK_SCRIPT)
        .map_err(|err| format!("failed to write {}: {err}", paths.hook_path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&paths.hook_path, fs::Permissions::from_mode(0o755))
            .map_err(|err| format!("failed to chmod {}: {err}", paths.hook_path.display()))?;
    }

    fs::write(&paths.skill_path, SKILL)
        .map_err(|err| format!("failed to write {}: {err}", paths.skill_path.display()))?;

    if settings_changed {
        atomic_write(&paths.settings_path, &updated)?;
    }

    Ok(InstallResult {
        mcp: register_mcp(),
        config_dir: paths.config_dir,
        hook_path: paths.hook_path,
        settings_path: paths.settings_path,
        skill_path: paths.skill_path,
    })
}

fn uninstall() -> Result<RevertResult, String> {
    let paths = ClaudePaths::resolve()?;
    let existing = read_settings(&paths.settings_path)?;
    let (updated, settings_changed) = if paths.settings_path.exists() {
        unmerge_settings(&existing)?
    } else {
        (String::new(), false)
    };

    if settings_changed {
        if updated.trim().is_empty() {
            let _ = fs::remove_file(&paths.settings_path);
        } else {
            atomic_write(&paths.settings_path, &updated)?;
        }
    }

    let hook_removed = if paths.hook_path.is_file() {
        fs::remove_file(&paths.hook_path)
            .map_err(|err| format!("failed to remove {}: {err}", paths.hook_path.display()))?;
        true
    } else {
        false
    };

    let skill_removed = if paths.skill_path.is_file() {
        fs::remove_file(&paths.skill_path)
            .map_err(|err| format!("failed to remove {}: {err}", paths.skill_path.display()))?;
        let _ = fs::remove_dir(&paths.skill_dir);
        true
    } else {
        false
    };

    Ok(RevertResult {
        mcp: unregister_mcp(),
        config_dir: paths.config_dir,
        hook_removed,
        skill_removed,
        settings_updated: settings_changed,
    })
}

fn claude_config_dir() -> Result<PathBuf, String> {
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

fn is_memorywhale_hook_command(command: &str) -> bool {
    command.starts_with("python3 \"") && command.ends_with("hooks/mw-record.py\"")
}

fn parse_settings(existing: &str) -> Result<ClaudeSettings, String> {
    if existing.trim().is_empty() {
        return Ok(ClaudeSettings::default());
    }
    let root: Value = serde_json::from_str(existing)
        .map_err(|err| format!("invalid Claude settings.json; file was not changed: {err}"))?;
    if !root.is_object() {
        return Err("invalid Claude settings.json; expected a top-level object".to_string());
    }
    serde_json::from_value(root)
        .map_err(|err| format!("invalid Claude settings.json; file was not changed: {err}"))
}

fn serialize_settings(root: &ClaudeSettings) -> Result<String, String> {
    if root.hooks.is_none() && root.extra.is_empty() {
        return Ok(String::new());
    }
    serde_json::to_string_pretty(root)
        .map(|s| format!("{s}\n"))
        .map_err(|err| format!("failed to serialize settings.json: {err}"))
}

fn merge_settings(existing: &str, hook_path: &Path) -> Result<(String, bool), String> {
    let before = parse_settings(existing)?;
    let mut root = before.clone();
    let entry = HookEntry::command(hook_path);

    let hooks = root.hooks.get_or_insert_with(Hooks::default);
    if let Some(group) = hooks
        .post_tool_use
        .iter_mut()
        .find(|group| group.matcher.as_deref() == Some("Bash"))
    {
        let list = group.hooks.get_or_insert_with(Vec::new);
        list.retain(|hook| !hook.is_memorywhale());
        list.push(entry);
    } else {
        hooks.post_tool_use.push(HookGroup {
            matcher: Some("Bash".to_string()),
            hooks: Some(vec![entry]),
            extra: Map::new(),
        });
    }

    if root == before {
        return Ok((existing.to_string(), false));
    }
    Ok((serialize_settings(&root)?, true))
}

fn unmerge_settings(existing: &str) -> Result<(String, bool), String> {
    if existing.trim().is_empty() {
        return Ok((String::new(), false));
    }

    let before = parse_settings(existing)?;
    let mut root = before.clone();
    let Some(hooks) = root.hooks.as_mut() else {
        return Ok((existing.to_string(), false));
    };

    hooks.post_tool_use.retain_mut(|group| {
        if group.matcher.as_deref() != Some("Bash") {
            return true;
        }
        let Some(list) = group.hooks.as_mut() else {
            return true;
        };
        list.retain(|hook| !hook.is_memorywhale());
        !list.is_empty()
    });
    if hooks.is_empty() {
        root.hooks = None;
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

fn register_mcp() -> McpOutcome {
    match Command::new("claude")
        .args(["mcp", "get", "--scope", "user", "memorywhale"])
        .output()
    {
        Ok(output) if output.status.success() => McpOutcome::Unchanged,
        Err(err) if err.kind() == ErrorKind::NotFound => McpOutcome::CliMissing,
        Err(_) => McpOutcome::Failed,
        Ok(_) => match Command::new("claude")
            .args([
                "mcp",
                "add",
                "--scope",
                "user",
                "--transport",
                "stdio",
                "memorywhale",
                "--",
                "mw-mcp",
            ])
            .status()
        {
            Ok(status) if status.success() => McpOutcome::Changed,
            Err(err) if err.kind() == ErrorKind::NotFound => McpOutcome::CliMissing,
            _ => McpOutcome::Failed,
        },
    }
}

fn unregister_mcp() -> McpOutcome {
    match Command::new("claude")
        .args(["mcp", "get", "--scope", "user", "memorywhale"])
        .output()
    {
        Ok(output) if output.status.success() => match Command::new("claude")
            .args(["mcp", "remove", "--scope", "user", "memorywhale"])
            .status()
        {
            Ok(status) if status.success() => McpOutcome::Changed,
            Err(err) if err.kind() == ErrorKind::NotFound => McpOutcome::CliMissing,
            _ => McpOutcome::Failed,
        },
        Ok(_) => McpOutcome::Unchanged,
        Err(err) if err.kind() == ErrorKind::NotFound => McpOutcome::CliMissing,
        Err(_) => McpOutcome::Failed,
    }
}

fn report_install(result: InstallResult) {
    println!("MemoryWhale installed for Claude Code.");
    println!("  config:   {}", result.config_dir.display());
    println!("  hook:     {}", result.hook_path.display());
    println!("  settings: {}", result.settings_path.display());
    println!("  skill:    {}", result.skill_path.display());
    match result.mcp {
        McpOutcome::Changed | McpOutcome::Unchanged => {
            println!("  mcp:      memorywhale registered (user scope)");
        }
        McpOutcome::CliMissing => {
            println!(
                "  mcp:      not registered — install the Claude Code CLI and run:\n\
                          {MCP_ADD}"
            );
        }
        McpOutcome::Failed => {
            println!(
                "  mcp:      not registered — `claude mcp add` failed. Run:\n          {MCP_ADD}"
            );
        }
    }
    println!("Restart Claude Code to pick up hook and skill changes.");
}

fn report_revert(result: RevertResult) {
    println!("MemoryWhale removed from Claude Code.");
    println!("  config:   {}", result.config_dir.display());
    if result.hook_removed {
        println!("  hook:     removed");
    }
    if result.skill_removed {
        println!("  skill:    removed");
    }
    if result.settings_updated {
        println!("  settings: MemoryWhale hook entry removed");
    }
    match result.mcp {
        McpOutcome::Changed => {
            println!("  mcp:      memorywhale unregistered (user scope)");
        }
        McpOutcome::Unchanged => {}
        McpOutcome::CliMissing | McpOutcome::Failed => {
            println!(
                "  mcp:      not unregistered — run manually if needed:\n            {MCP_REMOVE}"
            );
        }
    }
    println!("Restart Claude Code to pick up the change.");
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
        let existing = r#"{
  "hooks": {
    "PostToolUse": [
      {
        "matcher": "Bash",
        "hooks": [{"type": "command", "command": "python3 \"/old/home/.claude/hooks/mw-record.py\""}]
      }
    ]
  }
}"#;
        let (merged, changed) = merge_settings(existing, &hook).unwrap();
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
        let (reverted, changed) = unmerge_settings(&installed).unwrap();
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
        let (reverted, changed) = unmerge_settings(&installed).unwrap();
        assert!(changed);
        assert!(reverted.trim().is_empty());
    }

    #[test]
    fn unmerge_settings_is_unchanged_without_memorywhale_hook() {
        let original = r#"{"theme":"dark"}"#;
        let (updated, changed) = unmerge_settings(original).unwrap();
        assert!(!changed);
        assert_eq!(updated, original);
    }

    #[test]
    fn unmerge_settings_does_not_remove_unrelated_bash_hooks() {
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
        let (updated, changed) = unmerge_settings(original).unwrap();
        assert!(!changed);
        assert_eq!(updated, original);
    }
}

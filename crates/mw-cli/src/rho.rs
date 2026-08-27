//! Rho integration: capture hook, skill, and MCP registration.

use std::fs;
use std::path::{Path, PathBuf};

use toml_edit::{value, Array, ArrayOfTables, DocumentMut, Item, Table};

// In-crate so they ship inside the published package (same pattern as shell hooks).
const HOOK_SCRIPT: &str = include_str!("../rho/mw-record.py");
const SKILL: &str = include_str!("../rho/SKILL.md");

const HOOK_ID: &str = "memorywhale-record";
const HOOK_EVENT: &str = "after_tool_use";
const HOOK_TIMEOUT: &str = "15s";
const HOOK_TOOLS: [&str; 2] = ["bash", "powershell"];
const MCP_COMMAND: &str = "mw-mcp";
const MCP_TRANSPORT: &str = "stdio";

/// `mw integrate rho [--revert]`
pub fn cli(args: &[String]) -> Result<(), String> {
    let mut revert = false;
    for arg in args {
        match arg.as_str() {
            "--revert" => revert = true,
            _ => return Err("usage: mw integrate rho [--revert]".to_string()),
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
    hooks_path: PathBuf,
    settings_path: PathBuf,
    skill_path: PathBuf,
}

struct RevertResult {
    config_dir: PathBuf,
    hook_removed: bool,
    skill_removed: bool,
    hooks_updated: bool,
    mcp_updated: bool,
}

struct RhoPaths {
    config_dir: PathBuf,
    hook_path: PathBuf,
    hooks_path: PathBuf,
    skill_path: PathBuf,
    skill_dir: PathBuf,
    settings_path: PathBuf,
}

impl RhoPaths {
    fn resolve() -> Result<Self, String> {
        let config_dir = rho_home()?;
        let skill_dir = config_dir.join("skills/memorywhale");
        Ok(Self {
            hook_path: config_dir.join("hooks/mw-record.py"),
            hooks_path: config_dir.join("hooks.toml"),
            skill_path: skill_dir.join("SKILL.md"),
            settings_path: config_dir.join("config.toml"),
            skill_dir,
            config_dir,
        })
    }
}

fn install() -> Result<InstallResult, String> {
    let paths = RhoPaths::resolve()?;
    let existing_hooks = read_file(&paths.hooks_path)?;
    let existing_config = read_file(&paths.settings_path)?;
    let (hooks_updated, hooks_changed) = merge_hooks(&existing_hooks, &paths.hook_path)?;
    let (config_updated, config_changed) = merge_mcp(&existing_config)?;

    let hooks_dir = paths
        .hook_path
        .parent()
        .ok_or_else(|| format!("hook path has no parent: {}", paths.hook_path.display()))?;
    fs::create_dir_all(hooks_dir)
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

    if hooks_changed {
        atomic_write(&paths.hooks_path, &hooks_updated)?;
    }
    if config_changed {
        atomic_write(&paths.settings_path, &config_updated)?;
    }

    Ok(InstallResult {
        config_dir: paths.config_dir,
        hook_path: paths.hook_path,
        hooks_path: paths.hooks_path,
        settings_path: paths.settings_path,
        skill_path: paths.skill_path,
    })
}

fn uninstall() -> Result<RevertResult, String> {
    let paths = RhoPaths::resolve()?;
    let existing_hooks = read_file(&paths.hooks_path)?;
    let existing_config = read_file(&paths.settings_path)?;
    let (hooks_updated, hooks_changed) = if paths.hooks_path.exists() {
        unmerge_hooks(&existing_hooks)?
    } else {
        (String::new(), false)
    };
    let (config_updated, config_changed) = if paths.settings_path.exists() {
        unmerge_mcp(&existing_config)?
    } else {
        (String::new(), false)
    };

    if hooks_changed {
        write_or_remove(&paths.hooks_path, &hooks_updated)?;
    }
    if config_changed {
        write_or_remove(&paths.settings_path, &config_updated)?;
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
        config_dir: paths.config_dir,
        hook_removed,
        skill_removed,
        hooks_updated: hooks_changed,
        mcp_updated: config_changed,
    })
}

fn rho_home() -> Result<PathBuf, String> {
    if let Some(path) = std::env::var_os("RHO_HOME") {
        if !path.is_empty() {
            return Ok(PathBuf::from(path));
        }
    }
    dirs::home_dir()
        .ok_or_else(|| "could not resolve the home directory".to_string())
        .map(|home| home.join(".rho"))
}

fn read_file(path: &Path) -> Result<String, String> {
    if path.exists() {
        fs::read_to_string(path).map_err(|err| format!("failed to read {}: {err}", path.display()))
    } else {
        Ok(String::new())
    }
}

fn write_or_remove(path: &Path, contents: &str) -> Result<(), String> {
    if contents.trim().is_empty() {
        let _ = fs::remove_file(path);
        Ok(())
    } else {
        atomic_write(path, contents)
    }
}

fn parse_toml(existing: &str, what: &str) -> Result<DocumentMut, String> {
    if existing.trim().is_empty() {
        return Ok(DocumentMut::new());
    }
    existing
        .parse::<DocumentMut>()
        .map_err(|err| format!("invalid Rho {what}; file was not changed: {err}"))
}

fn hook_command(hook_path: &Path) -> [String; 2] {
    ["python3".to_string(), hook_path.display().to_string()]
}

fn string_array(table: &Table, key: &str) -> Option<Vec<String>> {
    table
        .get(key)
        .and_then(|item| item.as_array())
        .map(|array| {
            array
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect()
        })
}

fn set_string(table: &mut Table, key: &str, expected: &str) -> bool {
    if table.get(key).and_then(|item| item.as_str()) == Some(expected) {
        return false;
    }
    table[key] = value(expected);
    true
}

fn str_slice_eq(current: &[String], expected: &[&str]) -> bool {
    current.len() == expected.len() && current.iter().zip(expected.iter()).all(|(a, b)| a == b)
}

fn set_string_array(table: &mut Table, key: &str, expected: &[&str]) -> bool {
    if string_array(table, key).is_some_and(|current| str_slice_eq(&current, expected)) {
        return false;
    }
    let mut array = Array::new();
    for item in expected {
        array.push(*item);
    }
    table[key] = Item::Value(array.into());
    true
}

fn is_memorywhale_hook(table: &Table) -> bool {
    table.get("id").and_then(|item| item.as_str()) == Some(HOOK_ID)
}

fn hook_matches(table: &Table, hook_path: &Path) -> bool {
    let command = hook_command(hook_path);
    is_memorywhale_hook(table)
        && table.get("on").and_then(|item| item.as_str()) == Some(HOOK_EVENT)
        && string_array(table, "tools").is_some_and(|tools| str_slice_eq(&tools, &HOOK_TOOLS))
        && string_array(table, "command").is_some_and(|current| {
            str_slice_eq(&current, &[command[0].as_str(), command[1].as_str()])
        })
        && table.get("timeout").and_then(|item| item.as_str()) == Some(HOOK_TIMEOUT)
}

fn apply_hook_fields(table: &mut Table, hook_path: &Path) -> bool {
    let command = hook_command(hook_path);
    let command_refs = [command[0].as_str(), command[1].as_str()];
    let mut changed = false;
    changed |= set_string(table, "id", HOOK_ID);
    changed |= set_string(table, "on", HOOK_EVENT);
    changed |= set_string_array(table, "tools", &HOOK_TOOLS);
    changed |= set_string_array(table, "command", &command_refs);
    changed |= set_string(table, "timeout", HOOK_TIMEOUT);
    changed
}

fn require_hooks_version(doc: &DocumentMut) -> Result<(), String> {
    match doc.get("version") {
        None => Ok(()),
        Some(item) if item.as_integer() == Some(1) => Ok(()),
        Some(_) => Err(
            "unsupported Rho hooks.toml version; this installer writes version 1 and the file was not changed"
                .to_string(),
        ),
    }
}

fn merge_hooks(existing: &str, hook_path: &Path) -> Result<(String, bool), String> {
    let mut doc = parse_toml(existing, "hooks.toml")?;
    require_hooks_version(&doc)?;

    if !existing.trim().is_empty() {
        if let Some(hook) = doc.get("hook") {
            if !hook.is_array_of_tables() {
                return Err(
                    "invalid Rho hooks.toml; hook must be an array of tables and the file was not changed"
                        .to_string(),
                );
            }
        }
        if let Some(tables) = doc.get("hook").and_then(Item::as_array_of_tables) {
            if tables.iter().any(|table| hook_matches(table, hook_path))
                && doc.get("version").and_then(Item::as_integer) == Some(1)
            {
                return Ok((existing.to_string(), false));
            }
        }
    }

    let mut changed = false;
    if doc.get("version").and_then(Item::as_integer) != Some(1) {
        doc["version"] = value(1);
        changed = true;
    }

    if doc.get("hook").is_none() {
        doc["hook"] = Item::ArrayOfTables(ArrayOfTables::new());
        changed = true;
    }
    let hooks = doc
        .get_mut("hook")
        .and_then(Item::as_array_of_tables_mut)
        .ok_or_else(|| {
            "invalid Rho hooks.toml; hook must be an array of tables and the file was not changed"
                .to_string()
        })?;

    let mut existing_index = None;
    for index in 0..hooks.len() {
        if hooks.get(index).is_some_and(is_memorywhale_hook) {
            existing_index = Some(index);
            break;
        }
    }
    if let Some(index) = existing_index {
        changed |= apply_hook_fields(hooks.get_mut(index).expect("index from scan"), hook_path);
    } else {
        let mut table = Table::new();
        apply_hook_fields(&mut table, hook_path);
        hooks.push(table);
        changed = true;
    }

    if !changed {
        return Ok((existing.to_string(), false));
    }
    Ok((doc.to_string(), true))
}

fn unmerge_hooks(existing: &str) -> Result<(String, bool), String> {
    if existing.trim().is_empty() {
        return Ok((String::new(), false));
    }
    let mut doc = parse_toml(existing, "hooks.toml")?;
    require_hooks_version(&doc)?;

    let Some(hook) = doc.get_mut("hook") else {
        return Ok((existing.to_string(), false));
    };
    let Some(hooks) = hook.as_array_of_tables_mut() else {
        return Err(
            "invalid Rho hooks.toml; hook must be an array of tables and the file was not changed"
                .to_string(),
        );
    };
    let before = hooks.len();
    hooks.retain(|table| !is_memorywhale_hook(table));
    if hooks.len() == before {
        return Ok((existing.to_string(), false));
    }
    if hooks.is_empty() {
        doc.as_table_mut().remove("hook");
    }
    if doc.as_table().iter().all(|(key, _)| key == "version") {
        return Ok((String::new(), true));
    }
    Ok((doc.to_string(), true))
}

fn mcp_server_matches(doc: &DocumentMut) -> bool {
    doc.get("mcp")
        .and_then(|mcp| mcp.get("servers"))
        .and_then(|servers| servers.get("memorywhale"))
        .and_then(Item::as_table)
        .is_some_and(|server| {
            server.get("transport").and_then(Item::as_str) == Some(MCP_TRANSPORT)
                && server.get("command").and_then(Item::as_str) == Some(MCP_COMMAND)
        })
}

fn require_mcp_tables(doc: &DocumentMut) -> Result<(), String> {
    if let Some(mcp) = doc.get("mcp") {
        if !mcp.is_table() {
            return Err(
                "invalid Rho config.toml; mcp must be a table and the file was not changed"
                    .to_string(),
            );
        }
        if let Some(servers) = mcp.get("servers") {
            if !servers.is_table() {
                return Err(
                    "invalid Rho config.toml; mcp.servers must be a table and the file was not changed"
                        .to_string(),
                );
            }
            if let Some(server) = servers.get("memorywhale") {
                if !server.is_table() {
                    return Err(
                        "invalid Rho config.toml; mcp.servers.memorywhale must be a table and the file was not changed"
                            .to_string(),
                    );
                }
            }
        }
    }
    Ok(())
}

fn ensure_child_table(parent: &mut Table, key: &str, implicit: bool) -> Result<(), String> {
    match parent.get(key) {
        None => {
            let mut child = Table::new();
            child.set_implicit(implicit);
            parent.insert(key, Item::Table(child));
            Ok(())
        }
        Some(item) if item.is_table() => Ok(()),
        Some(_) => Err(format!(
            "invalid Rho config.toml; {key} must be a table and the file was not changed"
        )),
    }
}

fn memorywhale_server_table(doc: &mut DocumentMut) -> Result<&mut Table, String> {
    ensure_child_table(doc.as_table_mut(), "mcp", true)?;
    let mcp = doc
        .as_table_mut()
        .get_mut("mcp")
        .and_then(Item::as_table_mut)
        .ok_or_else(|| {
            "invalid Rho config.toml; mcp must be a table and the file was not changed".to_string()
        })?;
    ensure_child_table(mcp, "servers", true)?;
    let servers = mcp
        .get_mut("servers")
        .and_then(Item::as_table_mut)
        .ok_or_else(|| {
            "invalid Rho config.toml; mcp.servers must be a table and the file was not changed"
                .to_string()
        })?;
    ensure_child_table(servers, "memorywhale", false)?;
    servers
        .get_mut("memorywhale")
        .and_then(Item::as_table_mut)
        .ok_or_else(|| {
            "invalid Rho config.toml; mcp.servers.memorywhale must be a table and the file was not changed"
                .to_string()
        })
}

fn merge_mcp(existing: &str) -> Result<(String, bool), String> {
    let mut doc = parse_toml(existing, "config.toml")?;
    require_mcp_tables(&doc)?;
    if mcp_server_matches(&doc) {
        return Ok((existing.to_string(), false));
    }
    let server = memorywhale_server_table(&mut doc)?;
    let mut changed = false;
    changed |= set_string(server, "transport", MCP_TRANSPORT);
    changed |= set_string(server, "command", MCP_COMMAND);
    if !changed {
        return Ok((existing.to_string(), false));
    }
    Ok((doc.to_string(), true))
}

fn unmerge_mcp(existing: &str) -> Result<(String, bool), String> {
    if existing.trim().is_empty() {
        return Ok((String::new(), false));
    }
    let mut doc = parse_toml(existing, "config.toml")?;
    require_mcp_tables(&doc)?;

    let Some(mcp) = doc.get_mut("mcp") else {
        return Ok((existing.to_string(), false));
    };
    let Some(mcp_table) = mcp.as_table_mut() else {
        return Err(
            "invalid Rho config.toml; mcp must be a table and the file was not changed".to_string(),
        );
    };
    let Some(servers) = mcp_table.get_mut("servers") else {
        return Ok((existing.to_string(), false));
    };
    let Some(servers_table) = servers.as_table_mut() else {
        return Err(
            "invalid Rho config.toml; mcp.servers must be a table and the file was not changed"
                .to_string(),
        );
    };
    if servers_table.remove("memorywhale").is_none() {
        return Ok((existing.to_string(), false));
    }
    if servers_table.is_empty() {
        mcp_table.remove("servers");
    }
    if mcp_table.is_empty() {
        doc.as_table_mut().remove("mcp");
    }
    if doc.as_table().is_empty() {
        return Ok((String::new(), true));
    }
    Ok((doc.to_string(), true))
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

fn report_install(result: InstallResult) {
    println!("MemoryWhale installed for Rho.");
    println!("  config:   {}", result.config_dir.display());
    println!("  hook:     {}", result.hook_path.display());
    println!("  hooks:    {}", result.hooks_path.display());
    println!("  settings: {}", result.settings_path.display());
    println!("  skill:    {}", result.skill_path.display());
    println!("  mcp:      memorywhale registered in config.toml");
    println!("Restart Rho to pick up hook, skill, and MCP changes.");
}

fn report_revert(result: RevertResult) {
    println!("MemoryWhale removed from Rho.");
    println!("  config:   {}", result.config_dir.display());
    if result.hook_removed {
        println!("  hook:     removed");
    }
    if result.skill_removed {
        println!("  skill:    removed");
    }
    if result.hooks_updated {
        println!("  hooks:    MemoryWhale hook entry removed");
    }
    if result.mcp_updated {
        println!("  mcp:      memorywhale unregistered");
    }
    println!("Restart Rho to pick up the change.");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hook(path: &str) -> PathBuf {
        PathBuf::from(path)
    }

    #[test]
    fn merge_hooks_adds_entry_to_empty_config() {
        let path = hook("/home/me/.rho/hooks/mw-record.py");
        let (merged, changed) = merge_hooks("", &path).unwrap();
        assert!(changed);
        let doc: DocumentMut = merged.parse().unwrap();
        assert_eq!(doc["version"].as_integer(), Some(1));
        let table = doc["hook"].as_array_of_tables().unwrap().get(0).unwrap();
        assert!(hook_matches(table, &path));
    }

    #[test]
    fn merge_hooks_preserves_other_hooks_and_is_idempotent() {
        let path = hook("/tmp/.rho/hooks/mw-record.py");
        let original = r#"version = 1

[[hook]]
id = "fmt-rust"
on = "after_tool_use"
tools = ["edit", "write"]
command = ["./.rho/hooks/fmt-rust"]
timeout = "5s"
"#;
        let (once, changed_once) = merge_hooks(original, &path).unwrap();
        assert!(changed_once);
        let doc: DocumentMut = once.parse().unwrap();
        let hooks = doc["hook"].as_array_of_tables().unwrap();
        assert_eq!(hooks.len(), 2);
        assert_eq!(hooks.get(0).unwrap()["id"].as_str(), Some("fmt-rust"));

        let (twice, changed_twice) = merge_hooks(&once, &path).unwrap();
        assert!(!changed_twice);
        assert_eq!(twice, once);
    }

    #[test]
    fn merge_hooks_updates_stale_hook_path() {
        let path = hook("/new/home/.rho/hooks/mw-record.py");
        let existing = r#"version = 1

[[hook]]
id = "memorywhale-record"
on = "after_tool_use"
tools = ["bash", "powershell"]
command = ["python3", "/old/home/.rho/hooks/mw-record.py"]
timeout = "15s"
"#;
        let (merged, changed) = merge_hooks(existing, &path).unwrap();
        assert!(changed);
        let table = merged.parse::<DocumentMut>().unwrap()["hook"]
            .as_array_of_tables()
            .unwrap()
            .get(0)
            .unwrap()
            .clone();
        assert_eq!(
            string_array(&table, "command").unwrap()[1],
            path.display().to_string()
        );
    }

    #[test]
    fn merge_hooks_rejects_invalid_toml() {
        let err = merge_hooks("version = [", &hook("/tmp/hook.py")).unwrap_err();
        assert!(err.contains("invalid Rho hooks.toml"));
    }

    #[test]
    fn merge_hooks_rejects_unsupported_version() {
        let err = merge_hooks("version = 2\n", &hook("/tmp/hook.py")).unwrap_err();
        assert!(err.contains("unsupported Rho hooks.toml version"));
    }

    #[test]
    fn unmerge_hooks_removes_only_memorywhale_hook() {
        let path = hook("/tmp/.rho/hooks/mw-record.py");
        let (installed, _) = merge_hooks(
            r#"version = 1

[[hook]]
id = "fmt-rust"
on = "after_tool_use"
command = ["./fmt"]
timeout = "5s"
"#,
            &path,
        )
        .unwrap();
        let (reverted, changed) = unmerge_hooks(&installed).unwrap();
        assert!(changed);
        let doc: DocumentMut = reverted.parse().unwrap();
        let hooks = doc["hook"].as_array_of_tables().unwrap();
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks.get(0).unwrap()["id"].as_str(), Some("fmt-rust"));
    }

    #[test]
    fn unmerge_hooks_drops_empty_file() {
        let path = hook("/tmp/.rho/hooks/mw-record.py");
        let (installed, _) = merge_hooks("", &path).unwrap();
        let (reverted, changed) = unmerge_hooks(&installed).unwrap();
        assert!(changed);
        assert!(reverted.trim().is_empty());
    }

    #[test]
    fn unmerge_hooks_is_unchanged_without_memorywhale_hook() {
        let original = "version = 1\n";
        let (updated, changed) = unmerge_hooks(original).unwrap();
        assert!(!changed);
        assert_eq!(updated, original);
    }

    #[test]
    fn merge_mcp_adds_server_to_empty_config() {
        let (merged, changed) = merge_mcp("").unwrap();
        assert!(changed);
        let doc: DocumentMut = merged.parse().unwrap();
        let server = doc["mcp"]["servers"]["memorywhale"].as_table().unwrap();
        assert_eq!(server["transport"].as_str(), Some("stdio"));
        assert_eq!(server["command"].as_str(), Some("mw-mcp"));
    }

    #[test]
    fn merge_mcp_preserves_other_settings_and_is_idempotent() {
        let original = r#"# keep me
[model]
provider = "openai"

[mcp.servers.filesystem]
transport = "stdio"
command = "npx"
"#;
        let (once, changed_once) = merge_mcp(original).unwrap();
        assert!(changed_once);
        assert!(once.contains("# keep me"));
        assert!(once.contains("provider = \"openai\""));
        assert!(once.contains("command = \"npx\""));
        assert!(once.contains("memorywhale"));

        let (twice, changed_twice) = merge_mcp(&once).unwrap();
        assert!(!changed_twice);
        assert_eq!(twice, once);
    }

    #[test]
    fn merge_mcp_preserves_existing_server_env() {
        let original = r#"[mcp.servers.memorywhale]
transport = "stdio"
command = "old-mcp"
env = { MEMORYWHALE_DATA_DIR = "/custom" }
"#;
        let (merged, changed) = merge_mcp(original).unwrap();
        assert!(changed);
        assert!(merged.contains("MEMORYWHALE_DATA_DIR"));
        let server = merged.parse::<DocumentMut>().unwrap()["mcp"]["servers"]["memorywhale"]
            .as_table()
            .cloned()
            .unwrap();
        assert_eq!(server["command"].as_str(), Some("mw-mcp"));
        assert_eq!(server["transport"].as_str(), Some("stdio"));
    }

    #[test]
    fn merge_mcp_rejects_invalid_toml() {
        let err = merge_mcp("model = [").unwrap_err();
        assert!(err.contains("invalid Rho config.toml"));
    }

    #[test]
    fn unmerge_mcp_removes_only_memorywhale_server() {
        let (installed, _) = merge_mcp(
            r#"[model]
provider = "openai"

[mcp.servers.filesystem]
transport = "stdio"
command = "npx"
"#,
        )
        .unwrap();
        let (reverted, changed) = unmerge_mcp(&installed).unwrap();
        assert!(changed);
        assert!(reverted.contains("provider = \"openai\""));
        assert!(reverted.contains("filesystem"));
        assert!(!reverted.contains("memorywhale"));
    }

    #[test]
    fn unmerge_mcp_drops_empty_file() {
        let (installed, _) = merge_mcp("").unwrap();
        let (reverted, changed) = unmerge_mcp(&installed).unwrap();
        assert!(changed);
        assert!(reverted.trim().is_empty());
    }

    #[test]
    fn unmerge_mcp_is_unchanged_without_memorywhale() {
        let original = "provider = \"openai\"\n";
        let (updated, changed) = unmerge_mcp(original).unwrap();
        assert!(!changed);
        assert_eq!(updated, original);
    }
}

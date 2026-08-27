use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

/// Shared hook/skill layout used by Claude Code and Rho.
pub(crate) struct BundledLayout {
    pub config_dir: PathBuf,
    pub hook_path: PathBuf,
    pub skill_path: PathBuf,
    pub skill_dir: PathBuf,
}

impl BundledLayout {
    pub(crate) fn from_config_dir(config_dir: PathBuf) -> Self {
        let skill_dir = config_dir.join("skills/memorywhale");
        Self {
            hook_path: config_dir.join("hooks/mw-record.py"),
            skill_path: skill_dir.join("SKILL.md"),
            skill_dir,
            config_dir,
        }
    }
}

pub(crate) fn parse_revert(args: &[String], usage: &str) -> Result<bool, String> {
    let mut revert = false;
    for arg in args {
        match arg.as_str() {
            "--revert" => revert = true,
            _ => return Err(usage.to_string()),
        }
    }
    Ok(revert)
}

pub(crate) fn read_or_empty(path: &Path) -> Result<String, String> {
    if path.exists() {
        fs::read_to_string(path).map_err(|err| format!("failed to read {}: {err}", path.display()))
    } else {
        Ok(String::new())
    }
}

pub(crate) fn atomic_write(path: &Path, contents: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("path has no file name: {}", path.display()))?;
    let tmp = parent.join(format!(
        ".{}.tmp.{}",
        file_name.to_string_lossy(),
        std::process::id()
    ));
    fs::write(&tmp, contents).map_err(|err| format!("failed to write {}: {err}", tmp.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = path
            .metadata()
            .map(|meta| meta.permissions().mode())
            .unwrap_or(0o600);
        fs::set_permissions(&tmp, fs::Permissions::from_mode(mode))
            .map_err(|err| format!("failed to set permissions on {}: {err}", tmp.display()))?;
    }
    if let Err(err) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(format!("failed to write {}: {err}", path.display()));
    }
    Ok(())
}

pub(crate) fn write_or_remove(path: &Path, contents: &str) -> Result<(), String> {
    if contents.trim().is_empty() {
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
            Err(err) => Err(format!("failed to remove {}: {err}", path.display())),
        }
    } else {
        atomic_write(path, contents)
    }
}

pub(crate) fn install_bundled(
    layout: &BundledLayout,
    hook_script: &str,
    skill: &str,
) -> Result<(), String> {
    let hooks_dir = layout
        .hook_path
        .parent()
        .ok_or_else(|| format!("hook path has no parent: {}", layout.hook_path.display()))?;
    fs::create_dir_all(hooks_dir)
        .map_err(|err| format!("failed to create {}: {err}", hooks_dir.display()))?;
    fs::create_dir_all(&layout.skill_dir)
        .map_err(|err| format!("failed to create {}: {err}", layout.skill_dir.display()))?;
    fs::write(&layout.hook_path, hook_script)
        .map_err(|err| format!("failed to write {}: {err}", layout.hook_path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&layout.hook_path, fs::Permissions::from_mode(0o755))
            .map_err(|err| format!("failed to chmod {}: {err}", layout.hook_path.display()))?;
    }
    fs::write(&layout.skill_path, skill)
        .map_err(|err| format!("failed to write {}: {err}", layout.skill_path.display()))?;
    Ok(())
}

pub(crate) fn remove_bundled(layout: &BundledLayout) -> Result<(bool, bool), String> {
    let hook_removed = if layout.hook_path.is_file() {
        fs::remove_file(&layout.hook_path)
            .map_err(|err| format!("failed to remove {}: {err}", layout.hook_path.display()))?;
        true
    } else {
        false
    };
    let skill_removed = if layout.skill_path.is_file() {
        fs::remove_file(&layout.skill_path)
            .map_err(|err| format!("failed to remove {}: {err}", layout.skill_path.display()))?;
        let _ = fs::remove_dir(&layout.skill_dir);
        true
    } else {
        false
    };
    Ok((hook_removed, skill_removed))
}

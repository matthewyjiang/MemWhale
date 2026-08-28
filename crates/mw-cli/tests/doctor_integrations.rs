use std::process::Command;

fn mw_cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_mw"))
}

fn sandbox(name: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "mw-doctor-{name}-{}-{}",
        std::process::id(),
        std::thread::current().name().unwrap_or("test")
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn doctor(home: &std::path::Path, claude: &std::path::Path, rho: &std::path::Path) -> Command {
    let data = home.join("mw-data");
    std::fs::create_dir_all(&data).unwrap();
    let mut cmd = mw_cmd();
    cmd.args(["doctor"])
        .env("HOME", home)
        .env("CLAUDE_CONFIG_DIR", claude)
        .env("RHO_HOME", rho)
        .env("MEMORYWHALE_DATA_DIR", data)
        .env("PATH", "");
    cmd
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn client_block<'a>(text: &'a str, title: &str, next_title: &str) -> &'a str {
    let start = format!("  {title}\n");
    let after = text.split(&start).nth(1).unwrap_or("");
    if next_title.is_empty() {
        after
    } else {
        let next = format!("  {next_title}\n");
        after.split(&next).next().unwrap_or(after)
    }
}

fn assert_core_diagnostics_present(text: &str) {
    assert!(text.contains("MemoryWhale doctor"), "{text}");
    assert!(text.contains("data dir"), "{text}");
    assert!(text.contains("database"), "{text}");
    assert!(text.contains("recording"), "{text}");
    assert!(text.contains("auto-record"), "{text}");
    assert!(
        text.contains("ok   mcp:") || text.contains("WARN mcp:"),
        "generic mcp diagnostic missing: {text}"
    );
    assert!(text.contains("Integrations"), "{text}");
}

#[test]
fn doctor_honors_custom_claude_and_rho_dirs_and_ignores_home_decoys() {
    let home = sandbox("decoy-home");
    let claude = sandbox("claude-empty");
    let rho = sandbox("rho-empty");
    std::fs::create_dir_all(home.join(".claude/skills/memorywhale")).unwrap();
    std::fs::write(
        home.join(".claude/skills/memorywhale/SKILL.md"),
        "decoy skill",
    )
    .unwrap();
    std::fs::create_dir_all(home.join(".rho/skills/memorywhale")).unwrap();
    std::fs::write(home.join(".rho/skills/memorywhale/SKILL.md"), "decoy skill").unwrap();

    let output = doctor(&home, &claude, &rho)
        .output()
        .expect("run mw doctor");
    assert!(output.status.success(), "doctor failed: {output:?}");
    let text = stdout(&output);
    assert_core_diagnostics_present(&text);

    let claude_block = client_block(&text, "Claude Code", "Rho");
    assert!(
        claude_block.contains("MCP                 not configured; run `mw integrate claude`"),
        "{text}"
    );
    assert!(
        claude_block.contains("auto-capture hook   not installed; run `mw integrate claude`"),
        "{text}"
    );
    assert!(
        claude_block.contains("skill               not installed; run `mw integrate claude`"),
        "{text}"
    );

    let rho_block = client_block(&text, "Rho", "");
    assert!(
        rho_block.contains("MCP                 not configured; run `mw integrate rho`"),
        "{text}"
    );
    assert!(
        rho_block.contains("auto-capture hook   not installed; run `mw integrate rho`"),
        "{text}"
    );
    assert!(
        rho_block.contains("skill               not installed; run `mw integrate rho`"),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&home);
    let _ = std::fs::remove_dir_all(&claude);
    let _ = std::fs::remove_dir_all(&rho);
}

#[test]
fn doctor_reports_not_detected_when_client_configs_are_absent() {
    let home = sandbox("no-clients");
    let data = home.join("mw-data");
    std::fs::create_dir_all(&data).unwrap();

    let output = mw_cmd()
        .args(["doctor"])
        .env("HOME", &home)
        .env("MEMORYWHALE_DATA_DIR", &data)
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("RHO_HOME")
        .env("PATH", "")
        .output()
        .expect("run mw doctor");
    assert!(output.status.success(), "doctor failed: {output:?}");
    let text = stdout(&output);
    assert_core_diagnostics_present(&text);
    assert!(text.contains("Claude Code\n    not detected"), "{text}");
    assert!(text.contains("Rho\n    not detected"), "{text}");
    assert!(!text.contains("mw integrate claude"), "{text}");
    assert!(!text.contains("mw integrate rho"), "{text}");
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn doctor_ignores_empty_claude_config_dir() {
    let home = sandbox("empty-claude-env");
    let data = home.join("mw-data");
    std::fs::create_dir_all(&data).unwrap();

    let output = mw_cmd()
        .args(["doctor"])
        .env("HOME", &home)
        .env("MEMORYWHALE_DATA_DIR", &data)
        .env("CLAUDE_CONFIG_DIR", "")
        .env_remove("RHO_HOME")
        .env("PATH", "")
        .output()
        .expect("run mw doctor");
    assert!(output.status.success(), "doctor failed: {output:?}");
    let text = stdout(&output);
    assert!(text.contains("Claude Code\n    not detected"), "{text}");
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn doctor_reports_partial_stale_and_full_client_state() {
    let home = sandbox("mixed-home");
    let claude = sandbox("claude-full");
    let rho = sandbox("rho-partial");

    std::fs::write(
        claude.join(".claude.json"),
        r#"{"mcpServers":{"memorywhale":{"command":"mw-mcp","args":[]}}}"#,
    )
    .unwrap();
    let install = mw_cmd()
        .args(["integrate", "claude"])
        .env("HOME", &home)
        .env("CLAUDE_CONFIG_DIR", &claude)
        .env("PATH", "")
        .output()
        .expect("install claude");
    assert!(
        install.status.success(),
        "claude integrate failed: {install:?}"
    );

    std::fs::create_dir_all(rho.join("skills/memorywhale")).unwrap();
    std::fs::write(rho.join("skills/memorywhale/SKILL.md"), "skill").unwrap();
    std::fs::write(
        rho.join("hooks.toml"),
        r#"version = 1

[[hook]]
id = "memorywhale-record"
on = "after_tool_use"
tools = ["bash", "powershell"]
command = ["/old/home/.local/bin/mw-remember", "--from-hook", "rho"]
timeout = "15s"
"#,
    )
    .unwrap();

    let output = doctor(&home, &claude, &rho)
        .output()
        .expect("run mw doctor");
    assert!(output.status.success(), "doctor failed: {output:?}");
    let text = stdout(&output);
    assert_core_diagnostics_present(&text);

    let claude_block = text
        .split("  Rho\n")
        .next()
        .and_then(|chunk| chunk.split("  Claude Code\n").nth(1))
        .unwrap_or("");
    assert!(
        claude_block.contains("configured and reachable")
            || claude_block.contains("MCP                 configured"),
        "claude mcp: {text}"
    );
    assert!(
        claude_block.contains("auto-capture hook   installed"),
        "{text}"
    );
    assert!(
        claude_block.contains("skill               installed"),
        "{text}"
    );

    let rho_block = text.split("  Rho\n").nth(1).unwrap_or("");
    assert!(
        rho_block.contains("not configured; run `mw integrate rho`"),
        "{text}"
    );
    assert!(
        rho_block.contains("stale; run `mw integrate rho`"),
        "{text}"
    );
    assert!(
        rho_block.contains("skill               installed"),
        "{text}"
    );
    let _ = std::fs::remove_dir_all(&home);
    let _ = std::fs::remove_dir_all(&claude);
    let _ = std::fs::remove_dir_all(&rho);
}

#[test]
fn doctor_exit_status_stays_ok_when_integrations_are_missing() {
    let home = sandbox("exit-home");
    let claude = sandbox("exit-claude");
    let rho = sandbox("exit-rho");
    let output = doctor(&home, &claude, &rho)
        .output()
        .expect("run mw doctor");
    assert!(
        output.status.success(),
        "missing optional integrations must not fail doctor: {output:?}"
    );
    let _ = std::fs::remove_dir_all(&home);
    let _ = std::fs::remove_dir_all(&claude);
    let _ = std::fs::remove_dir_all(&rho);
}

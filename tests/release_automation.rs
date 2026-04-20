use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use tempfile::TempDir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn powershell() -> &'static str {
    "pwsh"
}

fn write_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, content)?;
    Ok(())
}

#[test]
fn generate_release_notes_prefers_history_entries() -> Result<()> {
    let temp = TempDir::new()?;
    let history_path = temp.path().join("release-history.psd1");
    let output_path = temp.path().join("release-body.md");

    write_file(
        &history_path,
        r#"@{
    Releases = @(
        @{
            Version = "0.1.0"
            Commit = "1111111111111111111111111111111111111111"
            Title = "Foundation"
            Notes = @("Initial release")
        }
        @{
            Version = "0.1.1"
            Commit = "2222222222222222222222222222222222222222"
            Title = "Second"
            Notes = @("Adds service commands", "Adds release notes")
        }
    )
}"#,
    )?;

    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(
            repo_root()
                .join("scripts")
                .join("generate-release-notes.ps1"),
        )
        .arg("-Version")
        .arg("v0.1.1")
        .arg("-HistoryPath")
        .arg(&history_path)
        .arg("-OutputPath")
        .arg(&output_path)
        .arg("-Repository")
        .arg("owner/repo")
        .output()
        .context("failed to run generate-release-notes.ps1")?;

    assert!(
        output.status.success(),
        "generate-release-notes failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = fs::read_to_string(&output_path)?;
    assert!(body.contains("Adds service commands"));
    assert!(body.contains("Adds release notes"));
    assert!(body.contains("https://github.com/owner/repo/compare/v0.1.0...v0.1.1"));

    Ok(())
}

#[test]
fn generate_release_notes_falls_back_to_planning_titles() -> Result<()> {
    let temp = TempDir::new()?;
    let history_path = temp.path().join("release-history.psd1");
    let backlog_path = temp.path().join("backlog.yaml");
    let output_path = temp.path().join("release-body.md");

    write_file(
        &history_path,
        r#"@{
    Releases = @(
        @{
            Version = "0.1.0"
            Commit = "1111111111111111111111111111111111111111"
            Title = "Foundation"
            Notes = @("Initial release")
        }
    )
}"#,
    )?;
    write_file(
        &backlog_path,
        r#"# === v0.1.1: Operator controls ===
- id: TASK-001
    title: Add Telegram control commands and service management
    status: done
    priority: P0
    target_version: v0.1.1
    repo: codex-channels
"#,
    )?;

    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(
            repo_root()
                .join("scripts")
                .join("generate-release-notes.ps1"),
        )
        .arg("-Version")
        .arg("0.1.1")
        .arg("-HistoryPath")
        .arg(&history_path)
        .arg("-BacklogPath")
        .arg(&backlog_path)
        .arg("-OutputPath")
        .arg(&output_path)
        .arg("-Repository")
        .arg("owner/repo")
        .output()
        .context("failed to run generate-release-notes.ps1")?;

    assert!(
        output.status.success(),
        "generate-release-notes failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = fs::read_to_string(&output_path)?;
    assert!(body.contains("Add Telegram control commands and service management"));
    assert!(body.contains("https://github.com/owner/repo/compare/v0.1.0...v0.1.1"));

    Ok(())
}

#[test]
fn bump_version_sync_only_updates_version_sources() -> Result<()> {
    let temp = TempDir::new()?;
    let cargo_toml_path = temp.path().join("Cargo.toml");
    let version_path = temp.path().join("VERSION");

    write_file(
        &cargo_toml_path,
        r#"[package]
name = "codex-telegram-bridge"
version = "0.1.0"
edition = "2024"
"#,
    )?;
    write_file(&version_path, "0.1.0")?;

    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(repo_root().join("scripts").join("bump-version.ps1"))
        .arg("-RepoRoot")
        .arg(temp.path())
        .arg("-Version")
        .arg("0.1.8")
        .arg("-SyncOnly")
        .output()
        .context("failed to run bump-version.ps1")?;

    assert!(
        output.status.success(),
        "bump-version failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let cargo_toml = fs::read_to_string(&cargo_toml_path)?;
    assert!(cargo_toml.contains("version = \"0.1.8\""));
    assert_eq!(fs::read_to_string(&version_path)?, "0.1.8");

    Ok(())
}

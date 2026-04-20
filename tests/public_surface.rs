use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};
use tempfile::tempdir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn powershell() -> &'static str {
    "pwsh"
}

#[test]
fn secret_surface_audit_script_succeeds_for_tracked_repo_state() -> Result<()> {
    let script_path = repo_root().join("scripts").join("audit-secret-surface.ps1");
    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(&script_path)
        .current_dir(repo_root())
        .output()
        .with_context(|| format!("failed to run {}", script_path.display()))?;

    assert!(
        output.status.success(),
        "audit-secret-surface failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

#[test]
fn public_surface_audit_script_succeeds_for_tracked_repo_state() -> Result<()> {
    let script_path = repo_root().join("scripts").join("audit-public-surface.ps1");
    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(&script_path)
        .current_dir(repo_root())
        .output()
        .with_context(|| format!("failed to run {}", script_path.display()))?;

    assert!(
        output.status.success(),
        "audit-public-surface failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

#[test]
fn secret_surface_audit_allows_placeholder_assignments() -> Result<()> {
    let repo = tempdir()?;
    initialize_git_repo(repo.path())?;
    let script_path = repo.path().join("audit-secret-surface.ps1");
    std::fs::copy(
        repo_root().join("scripts").join("audit-secret-surface.ps1"),
        &script_path,
    )?;
    std::fs::write(
        repo.path().join("README.md"),
        "TELEGRAM_BOT_TOKEN=<YOUR_TELEGRAM_BOT_TOKEN>\nLIVE_WORKSPACE=C:/path/to/workspace\n",
    )?;
    git_add(repo.path(), "README.md")?;

    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(&script_path)
        .current_dir(repo.path())
        .output()
        .with_context(|| format!("failed to run {}", script_path.display()))?;

    assert!(
        output.status.success(),
        "audit-secret-surface should allow placeholders: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

#[test]
fn secret_surface_audit_rejects_live_assignments() -> Result<()> {
    let repo = tempdir()?;
    initialize_git_repo(repo.path())?;
    let script_path = repo.path().join("audit-secret-surface.ps1");
    std::fs::copy(
        repo_root().join("scripts").join("audit-secret-surface.ps1"),
        &script_path,
    )?;
    std::fs::write(
        repo.path().join("README.md"),
        concat!("LIVE_TELEGRAM_CHAT_ID", "=8642321094\n"),
    )?;
    git_add(repo.path(), "README.md")?;

    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(&script_path)
        .current_dir(repo.path())
        .output()
        .with_context(|| format!("failed to run {}", script_path.display()))?;

    assert!(
        !output.status.success(),
        "audit-secret-surface should reject live assignments"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("tracked assignment"));

    Ok(())
}

#[test]
fn secret_surface_audit_rejects_bot_token_like_values() -> Result<()> {
    let repo = tempdir()?;
    initialize_git_repo(repo.path())?;
    let script_path = repo.path().join("audit-secret-surface.ps1");
    std::fs::copy(
        repo_root().join("scripts").join("audit-secret-surface.ps1"),
        &script_path,
    )?;
    std::fs::write(
        repo.path().join("README.md"),
        concat!(
            "Use this token: 123456789",
            ":ABCDEFGHIJKLMNOPQRSTUV1234567890\n"
        ),
    )?;
    git_add(repo.path(), "README.md")?;

    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(&script_path)
        .current_dir(repo.path())
        .output()
        .with_context(|| format!("failed to run {}", script_path.display()))?;

    assert!(
        !output.status.success(),
        "audit-secret-surface should reject token-like values"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("token-like value"));

    Ok(())
}

#[test]
fn live_planning_files_stay_untracked_and_examples_stay_tracked() -> Result<()> {
    let output = Command::new("git")
        .args(["ls-files"])
        .current_dir(repo_root())
        .output()
        .context("failed to list tracked files")?;

    assert!(
        output.status.success(),
        "git ls-files failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let tracked = String::from_utf8(output.stdout)?;
    assert!(tracked.contains("tasks/backlog.example.yaml"));
    assert!(tracked.contains("tasks/roadmap-title-ja.example.psd1"));
    assert!(tracked.contains("tasks/ROADMAP.example.md"));
    assert!(!tracked.contains("tasks/backlog.yaml"));
    assert!(!tracked.contains("tasks/roadmap-title-ja.psd1"));
    assert!(!tracked.contains("docs/project/ROADMAP.md"));

    Ok(())
}

fn initialize_git_repo(path: &std::path::Path) -> Result<()> {
    let output = Command::new("git")
        .args(["init"])
        .current_dir(path)
        .output()
        .context("failed to initialize git repo")?;
    assert!(
        output.status.success(),
        "git init failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

fn git_add(path: &std::path::Path, file: &str) -> Result<()> {
    let output = Command::new("git")
        .args(["add", file])
        .current_dir(path)
        .output()
        .with_context(|| format!("failed to add {file}"))?;
    assert!(
        output.status.success(),
        "git add failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    Ok(())
}

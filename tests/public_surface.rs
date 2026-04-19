use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn powershell() -> &'static str {
    "pwsh"
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

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
    repo: remotty
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
fn generate_release_notes_uses_backlog_env_override_when_no_argument_is_passed() -> Result<()> {
    let temp = TempDir::new()?;
    let history_path = temp.path().join("release-history.psd1");
    let planning_root = temp.path().join("planning-root");
    let explicit_backlog_path = temp.path().join("custom-backlog.yaml");
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
        &explicit_backlog_path,
        r#"# === v0.1.1: Operator controls ===
- id: TASK-001
    title: Use explicit backlog override
    status: done
    priority: P0
    target_version: v0.1.1
    repo: remotty
"#,
    )?;
    write_file(
        &planning_root.join("backlog.yaml"),
        r#"# === v0.1.1: Wrong source ===
- id: TASK-999
    title: Wrong backlog source
    status: done
    priority: P0
    target_version: v0.1.1
    repo: remotty
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
        .arg("-OutputPath")
        .arg(&output_path)
        .env("REMOTTY_PLANNING_ROOT", &planning_root)
        .env("REMOTTY_BACKLOG_PATH", &explicit_backlog_path)
        .output()
        .context("failed to run generate-release-notes.ps1 with env override")?;

    assert!(
        output.status.success(),
        "generate-release-notes failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let body = fs::read_to_string(&output_path)?;
    assert!(body.contains("Use explicit backlog override"));
    assert!(!body.contains("Wrong backlog source"));

    Ok(())
}

#[test]
fn bump_version_sync_only_updates_version_sources() -> Result<()> {
    let temp = TempDir::new()?;
    let cargo_toml_path = temp.path().join("Cargo.toml");
    let cargo_lock_path = temp.path().join("Cargo.lock");
    let version_path = temp.path().join("VERSION");

    write_file(
        &cargo_toml_path,
        r#"[package]
name = "remotty"
version = "0.1.0"
edition = "2024"

[dependencies]
clap = { version = "4", features = ["derive"] }
"#,
    )?;
    write_file(
        &cargo_lock_path,
        r#"[[package]]
name = "remotty"
version = "0.1.0"
dependencies = [
 "anyhow",
]

[[package]]
name = "clap"
version = "4.5.0"
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
    assert!(cargo_toml.contains("clap = { version = \"4\", features = [\"derive\"] }"));
    let cargo_lock = fs::read_to_string(&cargo_lock_path)?;
    assert!(cargo_lock.contains("name = \"remotty\"\nversion = \"0.1.8\""));
    assert!(cargo_lock.contains("name = \"clap\"\nversion = \"4.5.0\""));
    assert_eq!(fs::read_to_string(&version_path)?, "0.1.8");

    Ok(())
}

#[test]
fn bump_version_fails_when_planning_inputs_are_missing() -> Result<()> {
    let temp = TempDir::new()?;
    let planning_root = temp.path().join("planning-root");
    let cargo_toml_path = temp.path().join("Cargo.toml");
    let version_path = temp.path().join("VERSION");

    fs::create_dir_all(&planning_root)?;
    write_file(
        &cargo_toml_path,
        r#"[package]
name = "remotty"
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
        .env("REMOTTY_PLANNING_ROOT", &planning_root)
        .output()
        .context("failed to run bump-version.ps1 with missing planning inputs")?;

    assert!(
        !output.status.success(),
        "bump-version unexpectedly succeeded"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("backlog.yaml not found"));
    assert!(fs::read_to_string(&version_path)? == "0.1.0");

    let cargo_toml = fs::read_to_string(&cargo_toml_path)?;
    assert!(cargo_toml.contains("version = \"0.1.0\""));

    Ok(())
}

#[test]
fn release_workflow_runs_public_audits_before_publishing() -> Result<()> {
    let workflow = fs::read_to_string(
        repo_root()
            .join(".github")
            .join("workflows")
            .join("release.yml"),
    )?;
    let audit_step = workflow
        .find("Run public release audits")
        .context("release workflow should run public release audits")?;
    let prepare_step = workflow
        .find("Prepare release assets")
        .context("release workflow should prepare release assets")?;
    let publish_step = workflow
        .find("Publish release")
        .context("release workflow should publish release")?;

    assert!(audit_step < prepare_step);
    assert!(prepare_step < publish_step);
    for script in [
        "./scripts/audit-public-surface.ps1",
        "./scripts/audit-doc-terminology.ps1",
        "./scripts/audit-secret-surface.ps1",
    ] {
        assert!(
            workflow.contains(script),
            "missing release audit script: {script}"
        );
    }

    Ok(())
}

#[test]
fn bump_version_runs_public_audits_before_release_workflow() -> Result<()> {
    let script = fs::read_to_string(repo_root().join("scripts").join("bump-version.ps1"))?;
    let planning_validation = script
        .find("& $validatePlanningScript")
        .context("bump-version should validate planning inputs")?;
    let public_audit = script
        .find("& $auditPublicSurfaceScript")
        .context("bump-version should run public surface audit")?;
    let doc_audit = script
        .find("& $auditDocTerminologyScript")
        .context("bump-version should run documentation terminology audit")?;
    let secret_audit = script
        .find("& $auditSecretSurfaceScript")
        .context("bump-version should run secret surface audit")?;
    let git_branch = script
        .find("git switch -c $branch")
        .context("bump-version should create release branch")?;

    assert!(planning_validation < public_audit);
    assert!(public_audit < doc_audit);
    assert!(doc_audit < secret_audit);
    assert!(secret_audit < git_branch);

    Ok(())
}

#[test]
fn bump_version_checks_native_release_command_failures() -> Result<()> {
    let script = fs::read_to_string(repo_root().join("scripts").join("bump-version.ps1"))?;

    for command in [
        "git switch -c $branch",
        "git add VERSION Cargo.toml Cargo.lock",
        "git commit",
        "git push -u origin $branch",
        "gh pr create",
        "gh pr checks $prNumber --watch",
        "gh pr merge $prNumber",
        "git switch main",
        "git pull --ff-only origin main",
        "git tag $tag",
        "git push origin $tag",
        "gh release create $tag",
    ] {
        let command_position = script
            .find(command)
            .with_context(|| format!("missing native command: {command}"))?;
        let assertion = format!("Assert-NativeSuccess \"{command}\"");
        let assertion_position = script
            .find(&assertion)
            .with_context(|| format!("missing native success assertion: {assertion}"))?;

        assert!(
            command_position < assertion_position,
            "success assertion should follow native command: {command}"
        );
    }

    Ok(())
}

#[test]
fn bump_version_keeps_empty_backlog_updates_countable() -> Result<()> {
    let script = fs::read_to_string(repo_root().join("scripts").join("bump-version.ps1"))?;

    assert!(
        script.contains(
            "$updatedTaskIds = @(Update-ReleaseBacklogStatus -BacklogPath $backlogPath -Version $normalizedVersion)"
        ),
        "bump-version should keep empty backlog update results as an array"
    );
    assert!(script.contains("if ($updatedTaskIds.Count -gt 0)"));

    Ok(())
}

#[test]
fn release_preflight_allows_missing_title_map_when_backlog_exists() -> Result<()> {
    let temp = TempDir::new()?;
    let backlog_path = temp.path().join("backlog.yaml");
    let missing_title_path = temp.path().join("roadmap-title-ja.psd1");

    write_file(
        &backlog_path,
        r#"# === v0.1.8: Release ===
- id: TASK-001
    title: Keep backlog available
    status: done
    priority: P0
    target_version: v0.1.8
    repo: remotty
"#,
    )?;

    let script = format!(
        ". '{}' ; Assert-ReleasePlanningInputsExist -BacklogPath '{}' -RoadmapTitleJaPath '{}' ; 'ok'",
        repo_root()
            .join("scripts")
            .join("release-common.ps1")
            .display(),
        backlog_path.display(),
        missing_title_path.display(),
    );

    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-Command")
        .arg(&script)
        .output()
        .context("failed to run release preflight assertion")?;

    assert!(
        output.status.success(),
        "release preflight unexpectedly failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "ok");

    Ok(())
}

#[test]
fn bump_version_fails_when_title_map_is_invalid() -> Result<()> {
    let temp = TempDir::new()?;
    let planning_root = temp.path().join("planning-root");
    let cargo_toml_path = temp.path().join("Cargo.toml");
    let version_path = temp.path().join("VERSION");

    fs::create_dir_all(&planning_root)?;
    write_file(
        &cargo_toml_path,
        r#"[package]
name = "remotty"
version = "0.1.0"
edition = "2024"
"#,
    )?;
    write_file(&version_path, "0.1.0")?;
    write_file(
        &planning_root.join("backlog.yaml"),
        r#"# === v0.1.8: Release ===
- id: TASK-001
    title: Keep backlog available
    status: done
    priority: P0
    target_version: v0.1.8
    repo: remotty
"#,
    )?;
    write_file(
        &planning_root.join("roadmap-title-ja.psd1"),
        "@{\nVersionTitles =\n",
    )?;

    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(repo_root().join("scripts").join("bump-version.ps1"))
        .arg("-RepoRoot")
        .arg(temp.path())
        .arg("-Version")
        .arg("0.1.8")
        .env("REMOTTY_PLANNING_ROOT", &planning_root)
        .output()
        .context("failed to run bump-version.ps1 with invalid title map")?;

    assert!(
        !output.status.success(),
        "bump-version unexpectedly succeeded"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("roadmap-title-ja.psd1 is invalid"));
    assert_eq!(fs::read_to_string(&version_path)?, "0.1.0");

    let cargo_toml = fs::read_to_string(&cargo_toml_path)?;
    assert!(cargo_toml.contains("version = \"0.1.0\""));

    Ok(())
}

#[test]
fn bump_version_fails_when_backlog_is_invalid() -> Result<()> {
    let temp = TempDir::new()?;
    let planning_root = temp.path().join("planning-root");
    let cargo_toml_path = temp.path().join("Cargo.toml");
    let version_path = temp.path().join("VERSION");

    fs::create_dir_all(&planning_root)?;
    write_file(
        &cargo_toml_path,
        r#"[package]
name = "remotty"
version = "0.1.0"
edition = "2024"
"#,
    )?;
    write_file(&version_path, "0.1.0")?;
    write_file(
        &planning_root.join("backlog.yaml"),
        r#"# === v0.1.8: Release ===
- id: TASK-001
    title: Invalid backlog
    status: progress
    priority: P0
    target_version: 0.1.8
    repo: remotty
"#,
    )?;

    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(repo_root().join("scripts").join("bump-version.ps1"))
        .arg("-RepoRoot")
        .arg(temp.path())
        .arg("-Version")
        .arg("0.1.8")
        .env("REMOTTY_PLANNING_ROOT", &planning_root)
        .output()
        .context("failed to run bump-version.ps1 with invalid backlog")?;

    assert!(
        !output.status.success(),
        "bump-version unexpectedly succeeded"
    );

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("planning validation failed"));
    assert!(stderr.contains("invalid status"));
    assert_eq!(fs::read_to_string(&version_path)?, "0.1.0");

    let cargo_toml = fs::read_to_string(&cargo_toml_path)?;
    assert!(cargo_toml.contains("version = \"0.1.0\""));

    Ok(())
}

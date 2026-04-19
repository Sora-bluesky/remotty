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
fn sync_roadmap_generates_grouped_japanese_view() -> Result<()> {
    let temp = TempDir::new()?;
    let backlog_path = temp.path().join("backlog.yaml");
    let roadmap_path = temp.path().join("ROADMAP.md");
    let title_path = temp.path().join("roadmap-title-ja.psd1");

    write_file(
        &backlog_path,
        r#"# === v0.1.0: Bootstrap ===
- id: TASK-001
    title: Create bridge foundation
    status: done
    priority: P0
    target_version: v0.1.0
    repo: codex-channels

- id: TASK-002
    title: Add Windows service management commands
    status: active
    priority: P1
    target_version: v0.1.0
    repo: codex-channels

# === v0.2.0: Next capabilities ===
- id: TASK-003
    title: Implement completion checks follow-up flow
    status: backlog
    priority: P0
    target_version: v0.2.0
    repo: codex-channels
"#,
    )?;

    write_file(
        &title_path,
        r#"@{
    VersionTitles = @{
        "v0.1.0" = "基盤の立ち上げ"
    }
    TaskTitles = @{
        "TASK-002" = "Windows サービス管理コマンドを追加"
    }
}"#,
    )?;

    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(repo_root().join("scripts").join("sync-roadmap.ps1"))
        .arg("-BacklogPath")
        .arg(&backlog_path)
        .arg("-RoadmapPath")
        .arg(&roadmap_path)
        .arg("-RoadmapTitleJaPath")
        .arg(&title_path)
        .output()
        .context("failed to run sync-roadmap.ps1")?;

    assert!(
        output.status.success(),
        "sync-roadmap failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let roadmap = fs::read_to_string(&roadmap_path)?;
    assert!(roadmap.contains("# ロードマップ"));
    assert!(roadmap.contains("### v0.1.0: 基盤の立ち上げ"));
    assert!(roadmap.contains("| v0.1.0 | 2 |"));
    assert!(roadmap.contains("Windows サービス管理コマンドを追加"));
    assert!(roadmap.contains("completion checks follow-up flow を実装"));
    assert!(
        roadmap.contains("[====================] 100% (1/1)") || roadmap.contains("[==========")
    );

    Ok(())
}

#[test]
fn planning_paths_prefers_env_override() -> Result<()> {
    let temp = TempDir::new()?;
    let planning_root = temp.path().join("planning-root");
    let expected = planning_root.join("ROADMAP.md");
    fs::create_dir_all(&planning_root)?;
    fs::write(&expected, "# existing external roadmap\n")?;

    let script = format!(
        ". '{}' ; Resolve-CodexChannelsPlanningFilePath -RepoRoot '{}' -LocalRelativePath 'tasks/ROADMAP.example.md' -EnvironmentVariable 'CODEX_CHANNELS_ROADMAP_PATH' -DefaultFileName 'ROADMAP.md'",
        repo_root()
            .join("scripts")
            .join("planning-paths.ps1")
            .display(),
        repo_root().display()
    );

    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-Command")
        .arg(script)
        .env("CODEX_CHANNELS_PLANNING_ROOT", &planning_root)
        .output()
        .context("failed to run planning-paths.ps1")?;

    assert!(
        output.status.success(),
        "planning-paths failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let resolved = String::from_utf8(output.stdout)?.trim().to_owned();
    assert_eq!(resolved, expected.display().to_string());

    Ok(())
}

#[test]
fn setup_planning_creates_live_files_and_marker() -> Result<()> {
    let temp = TempDir::new()?;
    let planning_root = temp.path().join("planning-root");
    let marker_path = temp.path().join("planning-root.txt");

    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(repo_root().join("scripts").join("setup-planning.ps1"))
        .arg("-PlanningRoot")
        .arg(&planning_root)
        .arg("-MarkerPath")
        .arg(&marker_path)
        .output()
        .context("failed to run setup-planning.ps1")?;

    assert!(
        output.status.success(),
        "setup-planning failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(
        fs::read_to_string(&marker_path)?,
        planning_root.display().to_string()
    );
    assert!(planning_root.join("backlog.yaml").exists());
    assert!(planning_root.join("roadmap-title-ja.psd1").exists());
    assert!(planning_root.join("ROADMAP.md").exists());

    let roadmap = fs::read_to_string(planning_root.join("ROADMAP.md"))?;
    assert!(roadmap.contains("# ロードマップ"));
    assert!(roadmap.contains("ブリッジ基盤を作成"));

    Ok(())
}

#[test]
fn setup_planning_preserves_existing_live_files() -> Result<()> {
    let temp = TempDir::new()?;
    let planning_root = temp.path().join("planning-root");
    let marker_path = temp.path().join("planning-root.txt");
    fs::create_dir_all(&planning_root)?;

    let existing_backlog = planning_root.join("backlog.yaml");
    write_file(
        &existing_backlog,
        "# === v9.9.9: Custom ===\n- id: TASK-999\n    title: Keep custom backlog\n    status: active\n    priority: P0\n    target_version: v9.9.9\n    repo: codex-channels\n",
    )?;

    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(repo_root().join("scripts").join("setup-planning.ps1"))
        .arg("-PlanningRoot")
        .arg(&planning_root)
        .arg("-MarkerPath")
        .arg(&marker_path)
        .output()
        .context("failed to run setup-planning.ps1")?;

    assert!(
        output.status.success(),
        "setup-planning failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let backlog = fs::read_to_string(&existing_backlog)?;
    assert!(backlog.contains("Keep custom backlog"));

    let roadmap = fs::read_to_string(planning_root.join("ROADMAP.md"))?;
    assert!(roadmap.contains("Keep custom backlog"));

    Ok(())
}

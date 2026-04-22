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
fn gitleaks_workflow_keeps_ci_secret_scan_enabled() -> Result<()> {
    let workflow = std::fs::read_to_string(
        repo_root()
            .join(".github")
            .join("workflows")
            .join("gitleaks.yml"),
    )?;

    assert!(workflow.contains("gitleaks/gitleaks-action@v2"));
    assert!(workflow.contains("fetch-depth: 0"));
    assert!(workflow.contains("pull_request:"));
    assert!(workflow.contains("push:"));
    assert!(workflow.contains("GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}"));

    Ok(())
}

#[test]
fn npm_package_keeps_binary_install_contract() -> Result<()> {
    let package = std::fs::read_to_string(repo_root().join("package.json"))?;
    let installer = std::fs::read_to_string(repo_root().join("npm").join("install.js"))?;
    let wrapper = std::fs::read_to_string(repo_root().join("bin").join("remotty.js"))?;
    let release_workflow = std::fs::read_to_string(
        repo_root()
            .join(".github")
            .join("workflows")
            .join("release.yml"),
    )?;
    let readme = std::fs::read_to_string(repo_root().join("README.md"))?;
    let quickstart =
        std::fs::read_to_string(repo_root().join("docs").join("telegram-quickstart.md"))?;
    let development_doc = std::fs::read_to_string(repo_root().join("docs").join("development.md"))?;

    assert!(package.contains(r#""postinstall": "node npm/install.js""#));
    assert!(package.contains(r#""remotty": "bin/remotty.js""#));
    assert!(package.contains(r#""bridge.toml""#));
    assert!(package.contains(r#""plugins/""#));
    assert!(package.contains(r#"".agents/""#));
    assert!(installer.contains("remotty-x64.exe"));
    assert!(installer.contains("remotty-arm64.exe"));
    assert!(installer.contains("releases/download"));
    assert!(wrapper.contains("remotty.exe"));
    assert!(release_workflow.contains("actions/setup-node@v4"));
    assert!(release_workflow.contains("npm pack --pack-destination release"));
    assert!(release_workflow.contains("cp release/remotty-*.tgz release/remotty.tgz"));
    assert!(release_workflow.contains("NPM_TOKEN: ${{ secrets.NPM_TOKEN }}"));
    assert!(release_workflow.contains("npm publish ./release/remotty-*.tgz --access public"));
    assert!(readme.contains("docs/assets/hero.png"));
    assert!(quickstart.contains("npm install -g remotty"));
    assert!(readme.contains("Codex thread"));
    assert!(readme.contains("Telegram Quickstart"));
    assert!(readme.contains("Advanced CLI Mode"));
    assert!(quickstart.contains("/remotty-use-this-project"));
    assert!(!readme.contains("releases/latest/download/remotty.tgz"));
    assert!(development_doc.contains("NPM_TOKEN"));
    assert!(development_doc.contains("npm publish .\\release\\remotty.tgz"));

    Ok(())
}

#[test]
fn public_docs_explain_thread_setup_and_advanced_mode() -> Result<()> {
    let readme_ja = std::fs::read_to_string(repo_root().join("README.ja.md"))?;
    let quickstart =
        std::fs::read_to_string(repo_root().join("docs").join("telegram-quickstart.md"))?;
    let quickstart_ja =
        std::fs::read_to_string(repo_root().join("docs").join("telegram-quickstart.ja.md"))?;
    let exec_doc = std::fs::read_to_string(repo_root().join("docs").join("exec-transport.md"))?;
    let exec_doc_ja =
        std::fs::read_to_string(repo_root().join("docs").join("exec-transport.ja.md"))?;
    let upgrading = std::fs::read_to_string(repo_root().join("docs").join("upgrading.md"))?;
    let upgrading_ja = std::fs::read_to_string(repo_root().join("docs").join("upgrading.ja.md"))?;

    assert!(readme_ja.contains("Codex スレッド"));
    assert!(readme_ja.contains("Telegram クイックスタート"));
    assert!(readme_ja.contains("高度な CLI モード"));
    assert!(quickstart.contains("/remotty-sessions <thread_id>"));
    assert!(quickstart.contains("/remotty-use-this-project"));
    assert!(quickstart.contains("Codex CLI users run"));
    assert!(quickstart.contains("remotty config workspace upsert"));
    assert!(quickstart.contains("Codex App chat box"));
    assert!(quickstart.contains("Windows protected storage"));
    assert!(quickstart.contains("remotty local plugins"));
    assert!(!quickstart.contains("writable_roots"));
    assert!(!quickstart.contains("path = \"C:/Users/you/Documents/project\""));
    assert!(!quickstart.contains(".agents/plugins/marketplace.json"));
    assert!(quickstart_ja.contains("/remotty-sessions <thread_id>"));
    assert!(quickstart_ja.contains("/remotty-use-this-project"));
    assert!(quickstart_ja.contains("Codex CLI では"));
    assert!(quickstart_ja.contains("remotty config workspace upsert"));
    assert!(quickstart_ja.contains("Codex App のチャット欄"));
    assert!(quickstart_ja.contains("Windows の保護領域"));
    assert!(quickstart_ja.contains("remotty local plugins"));
    assert!(!quickstart_ja.contains("writable_roots"));
    assert!(!quickstart_ja.contains("path = \"C:/Users/you/Documents/project\""));
    assert!(!quickstart_ja.contains(".agents/plugins/marketplace.json"));
    assert!(exec_doc.contains("transport = \"exec\""));
    assert!(exec_doc_ja.contains("transport = \"exec\""));
    assert!(upgrading.contains("transport = \"app_server\""));
    assert!(upgrading_ja.contains("transport = \"app_server\""));

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
    assert_command_output_contains(&output, "tracked assignment");

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
    assert_command_output_contains(&output, "token-like value");

    Ok(())
}

#[test]
fn secret_surface_audit_rejects_telegram_bot_urls_with_embedded_tokens() -> Result<()> {
    let repo = tempdir()?;
    initialize_git_repo(repo.path())?;
    let script_path = repo.path().join("audit-secret-surface.ps1");
    std::fs::copy(
        repo_root().join("scripts").join("audit-secret-surface.ps1"),
        &script_path,
    )?;
    let token_like_value = format!("{}:{}", "123456789", "A".repeat(24));
    std::fs::write(
        repo.path().join("README.md"),
        format!(
            "Invoke-RestMethod \"https://api.telegram.org/bot{token_like_value}/getUpdates\"\n"
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
        "audit-secret-surface should reject Telegram bot URLs with embedded tokens"
    );
    assert_command_output_contains(&output, "embedded token");

    Ok(())
}

#[test]
fn doc_terminology_audit_rejects_release_history_terms() -> Result<()> {
    let repo = tempdir()?;
    initialize_git_repo(repo.path())?;
    let script_path = repo.path().join("audit-doc-terminology.ps1");
    std::fs::copy(
        repo_root()
            .join("scripts")
            .join("audit-doc-terminology.ps1"),
        &script_path,
    )?;
    std::fs::create_dir_all(repo.path().join("scripts"))?;
    std::fs::write(
        repo.path().join("scripts").join("release-history.psd1"),
        r#"@{
    Releases = @(
        @{
            Version = "0.1.0"
            Notes = @("Reviewed by claude-opus before release.")
        }
    )
}
"#,
    )?;

    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(&script_path)
        .current_dir(repo.path())
        .output()
        .with_context(|| format!("failed to run {}", script_path.display()))?;

    assert!(
        !output.status.success(),
        "audit-doc-terminology should reject release history terminology"
    );
    assert_command_output_contains(
        &output,
        "release-history.psd1 contains banned term 'claude-opus'",
    );

    Ok(())
}

#[test]
fn live_planning_files_and_task_contents_stay_untracked() -> Result<()> {
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
    assert!(!tracked.contains("tasks/README.md"));
    assert!(!tracked.contains("tasks/backlog.example.yaml"));
    assert!(!tracked.contains("tasks/roadmap-title-ja.example.psd1"));
    assert!(!tracked.contains("tasks/ROADMAP.example.md"));
    assert!(!tracked.contains("tasks/backlog.yaml"));
    assert!(!tracked.contains("tasks/roadmap-title-ja.psd1"));
    assert!(!tracked.contains(".github/release-doc-reviews/"));
    assert!(!tracked.contains("docs/project/ROADMAP.md"));

    Ok(())
}

#[test]
fn public_surface_audit_rejects_live_planning_files_present_in_repo() -> Result<()> {
    let repo = tempdir()?;
    initialize_git_repo(repo.path())?;
    let script_path = repo.path().join("audit-public-surface.ps1");
    std::fs::copy(
        repo_root().join("scripts").join("audit-public-surface.ps1"),
        &script_path,
    )?;
    std::fs::create_dir_all(repo.path().join("tasks"))?;
    std::fs::write(
        repo.path().join("tasks").join("README.md"),
        "internal helper\n",
    )?;
    std::fs::write(
        repo.path().join("tasks").join("backlog.yaml"),
        "- id: TASK-001\n",
    )?;
    std::fs::create_dir_all(repo.path().join("scripts"))?;
    for script in [
        "audit-doc-terminology.ps1",
        "audit-secret-surface.ps1",
        "planning-paths.ps1",
        "setup-planning.ps1",
        "sync-roadmap.ps1",
        "validate-planning.ps1",
    ] {
        std::fs::copy(
            repo_root().join("scripts").join(script),
            repo.path().join("scripts").join(script),
        )?;
    }
    for tracked_path in [
        "scripts/audit-doc-terminology.ps1",
        "scripts/audit-secret-surface.ps1",
        "scripts/planning-paths.ps1",
        "scripts/setup-planning.ps1",
        "scripts/sync-roadmap.ps1",
        "scripts/validate-planning.ps1",
    ] {
        git_add(repo.path(), tracked_path)?;
    }

    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(&script_path)
        .current_dir(repo.path())
        .output()
        .with_context(|| format!("failed to run {}", script_path.display()))?;

    assert!(
        !output.status.success(),
        "audit-public-surface should reject live planning files"
    );
    assert_command_output_contains(&output, "forbidden live file present in repo");

    Ok(())
}

#[test]
fn public_surface_audit_rejects_unexpected_tracked_task_files() -> Result<()> {
    let repo = tempdir()?;
    initialize_git_repo(repo.path())?;
    let script_path = repo.path().join("audit-public-surface.ps1");
    std::fs::copy(
        repo_root().join("scripts").join("audit-public-surface.ps1"),
        &script_path,
    )?;
    std::fs::create_dir_all(repo.path().join("tasks"))?;
    std::fs::write(
        repo.path().join("tasks").join("README.md"),
        "internal helper\n",
    )?;
    std::fs::write(
        repo.path().join("tasks").join("notes.md"),
        "private notes\n",
    )?;
    std::fs::create_dir_all(repo.path().join("scripts"))?;
    for script in [
        "audit-doc-terminology.ps1",
        "audit-secret-surface.ps1",
        "planning-paths.ps1",
        "setup-planning.ps1",
        "sync-roadmap.ps1",
        "validate-planning.ps1",
    ] {
        std::fs::copy(
            repo_root().join("scripts").join(script),
            repo.path().join("scripts").join(script),
        )?;
    }
    for tracked_path in [
        "tasks/notes.md",
        "scripts/audit-doc-terminology.ps1",
        "scripts/audit-secret-surface.ps1",
        "scripts/planning-paths.ps1",
        "scripts/setup-planning.ps1",
        "scripts/sync-roadmap.ps1",
        "scripts/validate-planning.ps1",
    ] {
        git_add(repo.path(), tracked_path)?;
    }

    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(&script_path)
        .current_dir(repo.path())
        .output()
        .with_context(|| format!("failed to run {}", script_path.display()))?;

    assert!(
        !output.status.success(),
        "audit-public-surface should reject tracked task artifacts"
    );
    assert_command_output_contains(&output, "unexpected tracked task file: tasks/notes.md");

    Ok(())
}

#[test]
fn public_surface_audit_rejects_release_doc_review_records() -> Result<()> {
    let repo = tempdir()?;
    initialize_git_repo(repo.path())?;
    let script_path = repo.path().join("audit-public-surface.ps1");
    std::fs::copy(
        repo_root().join("scripts").join("audit-public-surface.ps1"),
        &script_path,
    )?;
    std::fs::create_dir_all(repo.path().join(".github").join("release-doc-reviews"))?;
    std::fs::write(
        repo.path()
            .join(".github")
            .join("release-doc-reviews")
            .join("v0.1.0.psd1"),
        "@{ Reviewed = $true }\n",
    )?;
    std::fs::create_dir_all(repo.path().join("scripts"))?;
    for script in [
        "audit-doc-terminology.ps1",
        "audit-secret-surface.ps1",
        "planning-paths.ps1",
        "setup-planning.ps1",
        "sync-roadmap.ps1",
        "validate-planning.ps1",
    ] {
        std::fs::copy(
            repo_root().join("scripts").join(script),
            repo.path().join("scripts").join(script),
        )?;
    }
    for tracked_path in [
        ".github/release-doc-reviews/v0.1.0.psd1",
        "scripts/audit-doc-terminology.ps1",
        "scripts/audit-secret-surface.ps1",
        "scripts/planning-paths.ps1",
        "scripts/setup-planning.ps1",
        "scripts/sync-roadmap.ps1",
        "scripts/validate-planning.ps1",
    ] {
        git_add(repo.path(), tracked_path)?;
    }

    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(&script_path)
        .current_dir(repo.path())
        .output()
        .with_context(|| format!("failed to run {}", script_path.display()))?;

    assert!(
        !output.status.success(),
        "audit-public-surface should reject release doc review records"
    );
    assert_command_output_contains(
        &output,
        "forbidden tracked file: .github/release-doc-reviews",
    );

    Ok(())
}

#[test]
fn public_surface_audit_rejects_live_planning_files_anywhere_in_repo() -> Result<()> {
    let repo = tempdir()?;
    initialize_git_repo(repo.path())?;
    let script_path = repo.path().join("audit-public-surface.ps1");
    std::fs::copy(
        repo_root().join("scripts").join("audit-public-surface.ps1"),
        &script_path,
    )?;
    std::fs::create_dir_all(repo.path().join("tasks"))?;
    std::fs::write(
        repo.path().join("tasks").join("README.md"),
        "internal helper\n",
    )?;
    std::fs::create_dir_all(repo.path().join("private-planning"))?;
    std::fs::write(
        repo.path().join("private-planning").join("backlog.yaml"),
        "- id: TASK-001\n",
    )?;
    std::fs::create_dir_all(repo.path().join("scripts"))?;
    for script in [
        "audit-doc-terminology.ps1",
        "audit-secret-surface.ps1",
        "planning-paths.ps1",
        "setup-planning.ps1",
        "sync-roadmap.ps1",
        "validate-planning.ps1",
    ] {
        std::fs::copy(
            repo_root().join("scripts").join(script),
            repo.path().join("scripts").join(script),
        )?;
    }
    for tracked_path in [
        "scripts/audit-doc-terminology.ps1",
        "scripts/audit-secret-surface.ps1",
        "scripts/planning-paths.ps1",
        "scripts/setup-planning.ps1",
        "scripts/sync-roadmap.ps1",
        "scripts/validate-planning.ps1",
    ] {
        git_add(repo.path(), tracked_path)?;
    }

    let output = Command::new(powershell())
        .arg("-NoProfile")
        .arg("-File")
        .arg(&script_path)
        .current_dir(repo.path())
        .output()
        .with_context(|| format!("failed to run {}", script_path.display()))?;

    assert!(
        !output.status.success(),
        "audit-public-surface should reject in-repo planning roots"
    );
    assert_command_output_contains(&output, "forbidden live file present in repo");
    assert_command_output_contains(&output, "private-planning");

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

fn assert_command_output_contains(output: &std::process::Output, expected: &str) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let output_text = format!("{stdout}{stderr}");
    let normalized_output = normalize_command_output(&output_text);
    let normalized_expected = normalize_command_output(expected);
    assert!(
        normalized_output.contains(&normalized_expected),
        "expected command output to contain {expected:?}\nstatus: {}\nstdout:\n{stdout}\nstderr:\n{stderr}",
        output.status
    );
}

fn normalize_command_output(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_whitespace() && *character != '|')
        .collect()
}

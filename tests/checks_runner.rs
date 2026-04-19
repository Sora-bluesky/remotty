use std::fs;
use std::path::Path;

use anyhow::Result;
use codex_telegram_bridge::config::{CheckCommand, CheckProfile, Config, checks::run_profile};
use tempfile::tempdir;

#[test]
fn config_loads_named_check_profiles_with_structured_commands() -> Result<()> {
    let dir = tempdir()?;
    let config_path = dir.path().join("bridge.toml");
    fs::write(
        &config_path,
        r#"
[service]
run_mode = "console"
poll_timeout_sec = 30
shutdown_grace_sec = 15

[telegram]
token_secret_ref = "secret"
allowed_chat_types = ["private"]
admin_sender_ids = [1]

[codex]
binary = "codex"
model = "gpt-5.4"
sandbox = "workspace-write"
approval = "on-request"
profile = "default"

[storage]
db_path = "state/bridge.db"
state_dir = "state"
temp_dir = "state/tmp"
log_dir = "state/logs"

[policy]
default_mode = "completion_checks"
progress_edit_interval_ms = 5000
max_output_chars = 12000

[checks.profiles.quick]
[[checks.profiles.quick.commands]]
name = "fmt"
program = "cargo"
args = ["fmt", "--check"]
timeout_sec = 30

[[checks.profiles.quick.commands]]
name = "tests"
program = "cargo"
args = ["test", "checks_runner"]
timeout_sec = 90

[[workspaces]]
id = "main"
path = "C:/workspace"
writable_roots = ["C:/workspace"]
default_mode = "completion_checks"
continue_prompt = "continue"
checks_profile = "quick"
"#,
    )?;

    let config = Config::load(&config_path)?;
    let profile = config
        .checks
        .profiles
        .get("quick")
        .expect("missing profile");
    assert_eq!(config.default_workspace().checks_profile, "quick");
    assert_eq!(profile.commands.len(), 2);
    assert_eq!(profile.commands[0].name, "fmt");
    assert_eq!(profile.commands[0].program, "cargo");
    assert_eq!(profile.commands[0].args, vec!["fmt", "--check"]);
    assert_eq!(profile.commands[1].timeout_sec, 90);
    Ok(())
}

#[tokio::test]
async fn run_profile_reports_success_after_all_commands_pass() -> Result<()> {
    let dir = tempdir()?;
    let marker = dir.path().join("passed.txt");
    let profile = CheckProfile {
        commands: vec![
            success_command("preflight"),
            touch_command("write-marker", &marker),
        ],
    };

    let result = run_profile("smoke", &profile, dir.path()).await?;

    assert!(result.success);
    assert!(!result.timed_out);
    assert_eq!(result.completed_commands, 2);
    assert_eq!(result.total_commands, 2);
    assert_eq!(
        result.summary(),
        "completion checks passed for profile 'smoke' (2/2 commands)"
    );
    assert_eq!(fs::read_to_string(&marker)?, "ok");
    Ok(())
}

#[tokio::test]
async fn run_profile_stops_after_first_failure() -> Result<()> {
    let dir = tempdir()?;
    let skipped = dir.path().join("skipped.txt");
    let profile = CheckProfile {
        commands: vec![
            success_command("first"),
            failure_command("fail", 7),
            touch_command("should-not-run", &skipped),
        ],
    };

    let result = run_profile("smoke", &profile, dir.path()).await?;

    assert!(!result.success);
    assert!(!result.timed_out);
    assert_eq!(result.completed_commands, 1);
    assert_eq!(result.failed_command.as_deref(), Some("fail"));
    assert_eq!(result.exit_code, Some(7));
    assert_eq!(
        result.summary(),
        "completion checks failed on 'fail' in profile 'smoke' (exit code 7)"
    );
    assert!(!skipped.exists());
    Ok(())
}

#[tokio::test]
async fn run_profile_reports_timeout_and_stops() -> Result<()> {
    let dir = tempdir()?;
    let skipped = dir.path().join("after-timeout.txt");
    let profile = CheckProfile {
        commands: vec![
            sleep_command("slow", 2, 1),
            touch_command("should-not-run", &skipped),
        ],
    };

    let result = run_profile("timeouty", &profile, dir.path()).await?;

    assert!(!result.success);
    assert!(result.timed_out);
    assert_eq!(result.completed_commands, 0);
    assert_eq!(result.failed_command.as_deref(), Some("slow"));
    assert_eq!(result.exit_code, None);
    assert_eq!(
        result.summary(),
        "completion checks timed out on 'slow' in profile 'timeouty'"
    );
    assert!(!skipped.exists());
    Ok(())
}

fn success_command(name: &str) -> CheckCommand {
    if cfg!(windows) {
        CheckCommand {
            name: name.to_owned(),
            program: "cmd".to_owned(),
            args: vec!["/C".to_owned(), "exit 0".to_owned()],
            timeout_sec: 5,
        }
    } else {
        CheckCommand {
            name: name.to_owned(),
            program: "sh".to_owned(),
            args: vec!["-lc".to_owned(), "exit 0".to_owned()],
            timeout_sec: 5,
        }
    }
}

fn failure_command(name: &str, exit_code: i32) -> CheckCommand {
    if cfg!(windows) {
        CheckCommand {
            name: name.to_owned(),
            program: "cmd".to_owned(),
            args: vec!["/C".to_owned(), format!("exit {exit_code}")],
            timeout_sec: 5,
        }
    } else {
        CheckCommand {
            name: name.to_owned(),
            program: "sh".to_owned(),
            args: vec!["-lc".to_owned(), format!("exit {exit_code}")],
            timeout_sec: 5,
        }
    }
}

fn touch_command(name: &str, path: &Path) -> CheckCommand {
    if cfg!(windows) {
        CheckCommand {
            name: name.to_owned(),
            program: "powershell".to_owned(),
            args: vec![
                "-NoProfile".to_owned(),
                "-Command".to_owned(),
                format!(
                    "Set-Content -LiteralPath '{}' -Value 'ok' -NoNewline",
                    path.display()
                ),
            ],
            timeout_sec: 5,
        }
    } else {
        CheckCommand {
            name: name.to_owned(),
            program: "sh".to_owned(),
            args: vec![
                "-lc".to_owned(),
                format!("printf ok > '{}'", path.display()),
            ],
            timeout_sec: 5,
        }
    }
}

fn sleep_command(name: &str, seconds: u64, timeout_sec: u64) -> CheckCommand {
    if cfg!(windows) {
        CheckCommand {
            name: name.to_owned(),
            program: "powershell".to_owned(),
            args: vec![
                "-NoProfile".to_owned(),
                "-Command".to_owned(),
                format!("Start-Sleep -Seconds {seconds}"),
            ],
            timeout_sec,
        }
    } else {
        CheckCommand {
            name: name.to_owned(),
            program: "sh".to_owned(),
            args: vec!["-lc".to_owned(), format!("sleep {seconds}")],
            timeout_sec,
        }
    }
}

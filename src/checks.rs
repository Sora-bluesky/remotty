use std::path::Path;
use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::process::Command;
use tokio::time::{Duration, timeout};

use super::{CheckCommand, CheckProfile};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckRunSummary {
    pub profile_name: String,
    pub total_commands: usize,
    pub completed_commands: usize,
    pub success: bool,
    pub timed_out: bool,
    pub failed_command: Option<String>,
    pub exit_code: Option<i32>,
}

impl CheckRunSummary {
    pub fn summary(&self) -> String {
        if self.success {
            return format!(
                "completion checks passed for profile '{}' ({}/{} commands)",
                self.profile_name, self.completed_commands, self.total_commands
            );
        }

        let failed_command = self.failed_command.as_deref().unwrap_or("unknown");
        if self.timed_out {
            return format!(
                "completion checks timed out on '{}' in profile '{}'",
                failed_command, self.profile_name
            );
        }

        match self.exit_code {
            Some(exit_code) => format!(
                "completion checks failed on '{}' in profile '{}' (exit code {})",
                failed_command, self.profile_name, exit_code
            ),
            None => format!(
                "completion checks failed on '{}' in profile '{}'",
                failed_command, self.profile_name
            ),
        }
    }
}

pub async fn run_profile(
    profile_name: &str,
    profile: &CheckProfile,
    workspace_path: impl AsRef<Path>,
) -> Result<CheckRunSummary> {
    let workspace_path = workspace_path.as_ref();
    let total_commands = profile.commands.len();
    let mut completed_commands = 0;

    for command in &profile.commands {
        match run_command(command, workspace_path).await? {
            CommandOutcome::Passed => {
                completed_commands += 1;
            }
            CommandOutcome::Failed { exit_code } => {
                return Ok(CheckRunSummary {
                    profile_name: profile_name.to_owned(),
                    total_commands,
                    completed_commands,
                    success: false,
                    timed_out: false,
                    failed_command: Some(command.name.clone()),
                    exit_code,
                });
            }
            CommandOutcome::TimedOut => {
                return Ok(CheckRunSummary {
                    profile_name: profile_name.to_owned(),
                    total_commands,
                    completed_commands,
                    success: false,
                    timed_out: true,
                    failed_command: Some(command.name.clone()),
                    exit_code: None,
                });
            }
        }
    }

    Ok(CheckRunSummary {
        profile_name: profile_name.to_owned(),
        total_commands,
        completed_commands,
        success: true,
        timed_out: false,
        failed_command: None,
        exit_code: None,
    })
}

enum CommandOutcome {
    Passed,
    Failed { exit_code: Option<i32> },
    TimedOut,
}

async fn run_command(command: &CheckCommand, workspace_path: &Path) -> Result<CommandOutcome> {
    let mut child = Command::new(&command.program)
        .args(&command.args)
        .current_dir(workspace_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to spawn check '{}'", command.name))?;

    let status = match timeout(Duration::from_secs(command.timeout_sec), child.wait()).await {
        Ok(status) => {
            status.with_context(|| format!("failed to wait for check '{}'", command.name))?
        }
        Err(_) => {
            let _ = child.start_kill();
            let _ = child.wait().await;
            return Ok(CommandOutcome::TimedOut);
        }
    };

    if status.success() {
        Ok(CommandOutcome::Passed)
    } else {
        Ok(CommandOutcome::Failed {
            exit_code: status.code(),
        })
    }
}

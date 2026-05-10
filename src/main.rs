use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use remotty::cli::{
    CliCommand, ConfigCommand, DemoCommand, SecretCommand, ServiceCommand, TelegramCommand,
    parse_args,
};
use remotty::config::{Config, RunMode};
use remotty::config_workspace;
use remotty::demo_fakechat;
use remotty::engine;
use remotty::live_smoke::{self, SmokeScenario};
use remotty::service;
use remotty::telegram_cli;
use remotty::windows_secret::{delete_secret, load_secret, store_secret};
use tracing_subscriber::EnvFilter;

enum BridgeLaunchMode {
    Config,
    Console,
    Service,
}

#[tokio::main]
async fn main() -> Result<()> {
    match parse_args(std::env::args().skip(1))? {
        CliCommand::Run { config_path } => run_bridge(config_path, BridgeLaunchMode::Config).await,
        CliCommand::RemoteControl {
            config_path,
            workspace_path,
        } => run_remote_control(config_path, workspace_path).await,
        CliCommand::Config(ConfigCommand::WorkspaceUpsert {
            config_path,
            workspace_path,
        }) => {
            let result = config_workspace::upsert_workspace(config_path, workspace_path)?;
            println!(
                "workspace `{}` saved in {} for {}",
                result.workspace_id,
                result.config_path.display(),
                config_workspace::render_workspace_path(&result.workspace_path)
            );
            Ok(())
        }
        CliCommand::Demo(DemoCommand::Fakechat(options)) => {
            demo_fakechat::run_fakechat(options).await
        }
        CliCommand::Secret(SecretCommand::Set { key, value }) => {
            let path = store_secret(&key, &value)?;
            println!("stored secret at {}", path.display());
            Ok(())
        }
        CliCommand::Secret(SecretCommand::Delete { key }) => {
            delete_secret(&key)?;
            println!("deleted secret {key}");
            Ok(())
        }
        CliCommand::Service(ServiceCommand::Run { config_path }) => {
            run_bridge(config_path, BridgeLaunchMode::Service).await
        }
        CliCommand::Service(ServiceCommand::Install { config_path }) => {
            let config_path = service::install_service(config_path)?;
            println!(
                "installed windows service `{}` with config {}",
                service::service_name(),
                config_path.display()
            );
            Ok(())
        }
        CliCommand::Service(ServiceCommand::Uninstall) => {
            let service_name = service::cli_service_name()?;
            service::uninstall_service()?;
            println!("uninstalled windows service `{service_name}`");
            Ok(())
        }
        CliCommand::Service(ServiceCommand::Start) => {
            let service_name = service::cli_service_name()?;
            service::start_installed_service()?;
            println!("started windows service `{service_name}`");
            Ok(())
        }
        CliCommand::Service(ServiceCommand::Stop) => {
            let service_name = service::cli_service_name()?;
            service::stop_installed_service()?;
            println!("stopped windows service `{service_name}`");
            Ok(())
        }
        CliCommand::Service(ServiceCommand::Status) => {
            let service_name = service::cli_service_name()?;
            let status = service::installed_service_status()?;
            println!(
                "windows service `{}` status: {}",
                service_name,
                service::format_service_status(&status)
            );
            Ok(())
        }
        CliCommand::Telegram(TelegramCommand::Configure { config_path }) => {
            println!("{}", telegram_cli::configure(config_path).await?);
            Ok(())
        }
        CliCommand::Telegram(TelegramCommand::Pair { config_path }) => {
            println!("{}", telegram_cli::pair(config_path).await?);
            Ok(())
        }
        CliCommand::Telegram(TelegramCommand::AccessPair { code, config_path }) => {
            println!("{}", telegram_cli::access_pair(config_path, &code).await?);
            Ok(())
        }
        CliCommand::Telegram(TelegramCommand::PolicyAllowlist { config_path }) => {
            println!("{}", telegram_cli::policy_allowlist(config_path)?);
            Ok(())
        }
        CliCommand::Telegram(TelegramCommand::LiveEnvCheck { config_path }) => {
            println!("{}", telegram_cli::live_env_check(config_path).await?);
            Ok(())
        }
        CliCommand::Telegram(TelegramCommand::Sessions {
            filter,
            config_path,
        }) => {
            println!(
                "{}",
                telegram_cli::sessions(config_path, filter.as_deref()).await?
            );
            Ok(())
        }
        CliCommand::Telegram(TelegramCommand::Smoke {
            scenario,
            config_path,
        }) => {
            let scenario = match scenario {
                remotty::cli::TelegramSmokeScenario::ApprovalAccept => {
                    SmokeScenario::ApprovalAccept
                }
                remotty::cli::TelegramSmokeScenario::ApprovalDecline => {
                    SmokeScenario::ApprovalDecline
                }
            };
            println!("{}", live_smoke::run_smoke(config_path, scenario).await?);
            Ok(())
        }
    }
}

async fn run_remote_control(config_path: PathBuf, workspace_path: PathBuf) -> Result<()> {
    let result = config_workspace::upsert_workspace(&config_path, workspace_path)?;
    println!("{}", format_remote_control_setup_message(&result));

    let config = Config::load(&config_path)?;
    if !telegram_token_is_configured(&config) {
        println!(
            "Telegram bot token is needed once. Paste it below; remotty stores it in Windows protected storage."
        );
        println!("{}", telegram_cli::configure(config_path.clone()).await?);
    }

    run_bridge(config_path, BridgeLaunchMode::Console).await
}

fn format_remote_control_setup_message(result: &config_workspace::WorkspaceUpsertResult) -> String {
    [
        "Remote Control".to_owned(),
        format!(
            "Project `{}` is registered for Telegram remote control.",
            result.workspace_id
        ),
        format!("Config: {}", result.config_path.display()),
        format!(
            "Workspace: {}",
            config_workspace::render_workspace_path(&result.workspace_path)
        ),
        "Send a Telegram message to the bot. First-time users get a pairing code automatically."
            .to_owned(),
    ]
    .join("\n")
}

fn telegram_token_is_configured(config: &Config) -> bool {
    load_secret(&config.telegram.token_secret_ref)
        .map(|token| !token.trim().is_empty())
        .unwrap_or_else(|_| {
            std::env::var("TELEGRAM_BOT_TOKEN")
                .map(|token| !token.trim().is_empty())
                .unwrap_or(false)
        })
}

fn ensure_dirs(config: &Config) -> Result<()> {
    fs::create_dir_all(&config.storage.state_dir)?;
    fs::create_dir_all(&config.storage.temp_dir)?;
    fs::create_dir_all(&config.storage.log_dir)?;
    if let Some(parent) = config.storage.db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
}

async fn run_bridge(config_path: PathBuf, launch_mode: BridgeLaunchMode) -> Result<()> {
    let config = Config::load(&config_path)?;
    config_workspace::ensure_default_workspace_is_ready(&config)?;
    ensure_dirs(&config)?;
    init_tracing();

    match launch_mode {
        BridgeLaunchMode::Console => engine::run_console(config).await,
        BridgeLaunchMode::Service => service::run_service_mode(config),
        BridgeLaunchMode::Config => match config.service.run_mode {
            RunMode::Console => engine::run_console(config).await,
            RunMode::Service => service::run_service_mode(config),
        },
    }
}

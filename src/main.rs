use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use remotty::cli::{
    CliCommand, DemoCommand, SecretCommand, ServiceCommand, TelegramCommand, parse_args,
};
use remotty::config::{Config, RunMode};
use remotty::demo_fakechat;
use remotty::engine;
use remotty::live_smoke::{self, SmokeScenario};
use remotty::service;
use remotty::telegram_cli;
use remotty::windows_secret::{delete_secret, store_secret};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    match parse_args(std::env::args().skip(1))? {
        CliCommand::Run { config_path } => run_bridge(config_path, false).await,
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
            run_bridge(config_path, true).await
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

async fn run_bridge(config_path: PathBuf, force_service_mode: bool) -> Result<()> {
    let config = Config::load(&config_path)?;
    ensure_dirs(&config)?;
    init_tracing();

    if force_service_mode {
        service::run_service_mode(config)
    } else {
        match config.service.run_mode {
            RunMode::Console => engine::run_console(config).await,
            RunMode::Service => service::run_service_mode(config),
        }
    }
}

use std::fs;
use std::path::PathBuf;

use anyhow::Result;
use codex_telegram_bridge::cli::{CliCommand, SecretCommand, ServiceCommand, parse_args};
use codex_telegram_bridge::config::{Config, RunMode};
use codex_telegram_bridge::engine;
use codex_telegram_bridge::service;
use codex_telegram_bridge::windows_secret::{delete_secret, store_secret};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    match parse_args(std::env::args().skip(1))? {
        CliCommand::Run { config_path } => run_bridge(config_path, false).await,
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
            service::uninstall_service()?;
            println!("uninstalled windows service `{}`", service::service_name());
            Ok(())
        }
        CliCommand::Service(ServiceCommand::Start) => {
            service::start_installed_service()?;
            println!("started windows service `{}`", service::service_name());
            Ok(())
        }
        CliCommand::Service(ServiceCommand::Stop) => {
            service::stop_installed_service()?;
            println!("stopped windows service `{}`", service::service_name());
            Ok(())
        }
        CliCommand::Service(ServiceCommand::Status) => {
            let status = service::installed_service_status()?;
            println!(
                "windows service `{}` status: {}",
                service::service_name(),
                service::format_service_status(&status)
            );
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

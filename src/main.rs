mod codex;
mod config;
mod engine;
mod service;
mod store;
mod telegram;
mod windows_secret;

use std::fs;

use anyhow::Result;
use config::{Config, RunMode};
use tracing_subscriber::EnvFilter;
use windows_secret::{delete_secret, store_secret};

#[tokio::main]
async fn main() -> Result<()> {
    if handle_secret_commands()? {
        return Ok(());
    }

    let config = Config::load("bridge.toml")?;
    ensure_dirs(&config)?;
    init_tracing();

    match config.service.run_mode {
        RunMode::Console => engine::run_console(config).await,
        RunMode::Service => service::run_service_mode(config),
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

fn handle_secret_commands() -> Result<bool> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 || args[1] != "secret" {
        return Ok(false);
    }

    match args.get(2).map(String::as_str) {
        Some("set") => {
            let key = args
                .get(3)
                .ok_or_else(|| anyhow::anyhow!("missing secret key"))?;
            let value = args
                .get(4)
                .ok_or_else(|| anyhow::anyhow!("missing secret value"))?;
            let path = store_secret(key, value)?;
            println!("stored secret at {}", path.display());
        }
        Some("delete") => {
            let key = args
                .get(3)
                .ok_or_else(|| anyhow::anyhow!("missing secret key"))?;
            delete_secret(key)?;
            println!("deleted secret {key}");
        }
        _ => {
            println!("usage:");
            println!("  codex-telegram-bridge secret set <key> <value>");
            println!("  codex-telegram-bridge secret delete <key>");
        }
    }
    Ok(true)
}

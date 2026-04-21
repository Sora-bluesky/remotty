use std::path::PathBuf;

use anyhow::{Result, bail};

const DEFAULT_CONFIG_PATH: &str = "bridge.toml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    Run { config_path: PathBuf },
    Secret(SecretCommand),
    Service(ServiceCommand),
    Telegram(TelegramCommand),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretCommand {
    Set { key: String, value: String },
    Delete { key: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServiceCommand {
    Run { config_path: PathBuf },
    Install { config_path: PathBuf },
    Uninstall,
    Start,
    Stop,
    Status,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TelegramCommand {
    Configure {
        config_path: PathBuf,
    },
    Pair {
        config_path: PathBuf,
    },
    AccessPair {
        code: String,
        config_path: PathBuf,
    },
    PolicyAllowlist {
        config_path: PathBuf,
    },
    LiveEnvCheck {
        config_path: PathBuf,
    },
    Smoke {
        scenario: TelegramSmokeScenario,
        config_path: PathBuf,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelegramSmokeScenario {
    ApprovalAccept,
    ApprovalDecline,
}

pub fn parse_args(args: impl IntoIterator<Item = String>) -> Result<CliCommand> {
    let args = args.into_iter().collect::<Vec<_>>();
    if args.is_empty() {
        return Ok(CliCommand::Run {
            config_path: PathBuf::from(DEFAULT_CONFIG_PATH),
        });
    }

    match args[0].as_str() {
        "secret" => parse_secret_command(&args[1..]),
        "service" => parse_service_command(&args[1..]),
        "telegram" => parse_telegram_command(&args[1..]),
        "--config" => {
            let config_path = args
                .get(1)
                .ok_or_else(|| anyhow::anyhow!("missing config path after --config"))?;
            if args.len() != 2 {
                bail!("unexpected arguments after --config");
            }
            Ok(CliCommand::Run {
                config_path: PathBuf::from(config_path),
            })
        }
        other => bail!("unknown command: {other}"),
    }
}

fn parse_telegram_command(args: &[String]) -> Result<CliCommand> {
    match args {
        [action] if action == "configure" => Ok(CliCommand::Telegram(TelegramCommand::Configure {
            config_path: PathBuf::from(DEFAULT_CONFIG_PATH),
        })),
        [action, flag, value] if action == "configure" && flag == "--config" => {
            Ok(CliCommand::Telegram(TelegramCommand::Configure {
                config_path: PathBuf::from(value),
            }))
        }
        [action] if action == "pair" => Ok(CliCommand::Telegram(TelegramCommand::Pair {
            config_path: PathBuf::from(DEFAULT_CONFIG_PATH),
        })),
        [action, flag, value] if action == "pair" && flag == "--config" => {
            Ok(CliCommand::Telegram(TelegramCommand::Pair {
                config_path: PathBuf::from(value),
            }))
        }
        [action, code] if action == "access-pair" => {
            Ok(CliCommand::Telegram(TelegramCommand::AccessPair {
                code: code.clone(),
                config_path: PathBuf::from(DEFAULT_CONFIG_PATH),
            }))
        }
        [action, code, flag, value] if action == "access-pair" && flag == "--config" => {
            Ok(CliCommand::Telegram(TelegramCommand::AccessPair {
                code: code.clone(),
                config_path: PathBuf::from(value),
            }))
        }
        [group, action] if group == "policy" && action == "allowlist" => {
            Ok(CliCommand::Telegram(TelegramCommand::PolicyAllowlist {
                config_path: PathBuf::from(DEFAULT_CONFIG_PATH),
            }))
        }
        [group, action, flag, value]
            if group == "policy" && action == "allowlist" && flag == "--config" =>
        {
            Ok(CliCommand::Telegram(TelegramCommand::PolicyAllowlist {
                config_path: PathBuf::from(value),
            }))
        }
        [action] if action == "live-env-check" => {
            Ok(CliCommand::Telegram(TelegramCommand::LiveEnvCheck {
                config_path: PathBuf::from(DEFAULT_CONFIG_PATH),
            }))
        }
        [action, flag, value] if action == "live-env-check" && flag == "--config" => {
            Ok(CliCommand::Telegram(TelegramCommand::LiveEnvCheck {
                config_path: PathBuf::from(value),
            }))
        }
        [action, subaction, scenario] if action == "smoke" && subaction == "approval" => {
            Ok(CliCommand::Telegram(TelegramCommand::Smoke {
                scenario: parse_smoke_scenario(scenario)?,
                config_path: PathBuf::from(DEFAULT_CONFIG_PATH),
            }))
        }
        [action, subaction, scenario, flag, value]
            if action == "smoke" && subaction == "approval" && flag == "--config" =>
        {
            Ok(CliCommand::Telegram(TelegramCommand::Smoke {
                scenario: parse_smoke_scenario(scenario)?,
                config_path: PathBuf::from(value),
            }))
        }
        _ => bail!(
            "usage: telegram configure [--config <path>] | telegram pair [--config <path>] | telegram access-pair <code> [--config <path>] | telegram policy allowlist [--config <path>] | telegram live-env-check [--config <path>] | telegram smoke approval <accept|decline> [--config <path>]"
        ),
    }
}

fn parse_smoke_scenario(value: &str) -> Result<TelegramSmokeScenario> {
    match value {
        "accept" => Ok(TelegramSmokeScenario::ApprovalAccept),
        "decline" => Ok(TelegramSmokeScenario::ApprovalDecline),
        other => bail!("unknown smoke scenario: {other}"),
    }
}

fn parse_secret_command(args: &[String]) -> Result<CliCommand> {
    match args {
        [action, key, value] if action == "set" => Ok(CliCommand::Secret(SecretCommand::Set {
            key: key.clone(),
            value: value.clone(),
        })),
        [action, key] if action == "delete" => Ok(CliCommand::Secret(SecretCommand::Delete {
            key: key.clone(),
        })),
        _ => bail!("usage: secret set <key> <value> | secret delete <key>"),
    }
}

fn parse_service_command(args: &[String]) -> Result<CliCommand> {
    match args {
        [action] if action == "uninstall" => Ok(CliCommand::Service(ServiceCommand::Uninstall)),
        [action] if action == "start" => Ok(CliCommand::Service(ServiceCommand::Start)),
        [action] if action == "stop" => Ok(CliCommand::Service(ServiceCommand::Stop)),
        [action] if action == "status" => Ok(CliCommand::Service(ServiceCommand::Status)),
        [action] if action == "install" => Ok(CliCommand::Service(ServiceCommand::Install {
            config_path: PathBuf::from(DEFAULT_CONFIG_PATH),
        })),
        [action, flag, value] if action == "install" && flag == "--config" => {
            Ok(CliCommand::Service(ServiceCommand::Install {
                config_path: PathBuf::from(value),
            }))
        }
        [action, flag, value] if action == "run" && flag == "--config" => {
            Ok(CliCommand::Service(ServiceCommand::Run {
                config_path: PathBuf::from(value),
            }))
        }
        _ => bail!(
            "usage: service install [--config <path>] | service run --config <path> | service start | service stop | service status | service uninstall"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CliCommand, SecretCommand, ServiceCommand, TelegramCommand, TelegramSmokeScenario,
        parse_args,
    };
    use std::path::PathBuf;

    #[test]
    fn parse_args_defaults_to_running_bridge() {
        assert_eq!(
            parse_args(Vec::<String>::new()).expect("default command should parse"),
            CliCommand::Run {
                config_path: PathBuf::from("bridge.toml"),
            }
        );
    }

    #[test]
    fn parse_args_supports_global_config_override() {
        assert_eq!(
            parse_args(vec!["--config".to_owned(), "custom.toml".to_owned()])
                .expect("config override should parse"),
            CliCommand::Run {
                config_path: PathBuf::from("custom.toml"),
            }
        );
    }

    #[test]
    fn parse_args_supports_secret_set() {
        assert_eq!(
            parse_args(vec![
                "secret".to_owned(),
                "set".to_owned(),
                "bot".to_owned(),
                "token".to_owned(),
            ])
            .expect("secret set should parse"),
            CliCommand::Secret(SecretCommand::Set {
                key: "bot".to_owned(),
                value: "token".to_owned(),
            })
        );
    }

    #[test]
    fn parse_args_supports_service_install_with_config() {
        assert_eq!(
            parse_args(vec![
                "service".to_owned(),
                "install".to_owned(),
                "--config".to_owned(),
                "prod/bridge.toml".to_owned(),
            ])
            .expect("service install should parse"),
            CliCommand::Service(ServiceCommand::Install {
                config_path: PathBuf::from("prod/bridge.toml"),
            })
        );
    }

    #[test]
    fn parse_args_supports_service_status() {
        assert_eq!(
            parse_args(vec!["service".to_owned(), "status".to_owned()])
                .expect("service status should parse"),
            CliCommand::Service(ServiceCommand::Status)
        );
    }

    #[test]
    fn parse_args_rejects_incomplete_service_run() {
        let error = parse_args(vec!["service".to_owned(), "run".to_owned()])
            .expect_err("service run without config should fail");
        assert!(error.to_string().contains("usage: service"));
    }

    #[test]
    fn parse_args_supports_telegram_configure() {
        assert_eq!(
            parse_args(vec!["telegram".to_owned(), "configure".to_owned()])
                .expect("telegram configure should parse"),
            CliCommand::Telegram(TelegramCommand::Configure {
                config_path: PathBuf::from("bridge.toml"),
            })
        );
    }

    #[test]
    fn parse_args_supports_telegram_pair_with_config() {
        assert_eq!(
            parse_args(vec![
                "telegram".to_owned(),
                "pair".to_owned(),
                "--config".to_owned(),
                "prod/bridge.toml".to_owned(),
            ])
            .expect("telegram pair should parse"),
            CliCommand::Telegram(TelegramCommand::Pair {
                config_path: PathBuf::from("prod/bridge.toml"),
            })
        );
    }

    #[test]
    fn parse_args_supports_telegram_access_pair_with_config() {
        assert_eq!(
            parse_args(vec![
                "telegram".to_owned(),
                "access-pair".to_owned(),
                "ABC123".to_owned(),
                "--config".to_owned(),
                "prod/bridge.toml".to_owned(),
            ])
            .expect("telegram access-pair should parse"),
            CliCommand::Telegram(TelegramCommand::AccessPair {
                code: "ABC123".to_owned(),
                config_path: PathBuf::from("prod/bridge.toml"),
            })
        );
    }

    #[test]
    fn parse_args_supports_telegram_policy_allowlist() {
        assert_eq!(
            parse_args(vec![
                "telegram".to_owned(),
                "policy".to_owned(),
                "allowlist".to_owned(),
            ])
            .expect("telegram policy allowlist should parse"),
            CliCommand::Telegram(TelegramCommand::PolicyAllowlist {
                config_path: PathBuf::from("bridge.toml"),
            })
        );
    }

    #[test]
    fn parse_args_supports_live_env_check() {
        assert_eq!(
            parse_args(vec!["telegram".to_owned(), "live-env-check".to_owned(),])
                .expect("telegram live-env-check should parse"),
            CliCommand::Telegram(TelegramCommand::LiveEnvCheck {
                config_path: PathBuf::from("bridge.toml"),
            })
        );
    }

    #[test]
    fn parse_args_supports_live_env_check_with_config() {
        assert_eq!(
            parse_args(vec![
                "telegram".to_owned(),
                "live-env-check".to_owned(),
                "--config".to_owned(),
                "custom.toml".to_owned(),
            ])
            .expect("telegram live-env-check --config should parse"),
            CliCommand::Telegram(TelegramCommand::LiveEnvCheck {
                config_path: PathBuf::from("custom.toml"),
            })
        );
    }

    #[test]
    fn parse_args_supports_smoke_accept() {
        assert_eq!(
            parse_args(vec![
                "telegram".to_owned(),
                "smoke".to_owned(),
                "approval".to_owned(),
                "accept".to_owned(),
            ])
            .expect("telegram smoke approval accept should parse"),
            CliCommand::Telegram(TelegramCommand::Smoke {
                scenario: TelegramSmokeScenario::ApprovalAccept,
                config_path: PathBuf::from("bridge.toml"),
            })
        );
    }

    #[test]
    fn parse_args_supports_smoke_decline_with_config() {
        assert_eq!(
            parse_args(vec![
                "telegram".to_owned(),
                "smoke".to_owned(),
                "approval".to_owned(),
                "decline".to_owned(),
                "--config".to_owned(),
                "prod/bridge.toml".to_owned(),
            ])
            .expect("telegram smoke approval decline should parse"),
            CliCommand::Telegram(TelegramCommand::Smoke {
                scenario: TelegramSmokeScenario::ApprovalDecline,
                config_path: PathBuf::from("prod/bridge.toml"),
            })
        );
    }
}

use std::path::PathBuf;

use anyhow::{Result, bail};

const DEFAULT_CONFIG_PATH: &str = "bridge.toml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliCommand {
    Run { config_path: PathBuf },
    Secret(SecretCommand),
    Service(ServiceCommand),
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
    use super::{CliCommand, SecretCommand, ServiceCommand, parse_args};
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
}

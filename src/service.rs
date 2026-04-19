use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use service_manager::{
    RestartPolicy, ServiceInstallCtx, ServiceLabel, ServiceStartCtx,
    ServiceStatus as ManagedServiceStatus, ServiceStatusCtx, ServiceStopCtx, ServiceUninstallCtx,
    native_service_manager,
};
use tokio::runtime::Builder;
use tokio_util::sync::CancellationToken;
use windows_service::define_windows_service;
use windows_service::service::{
    ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult};
use windows_service::service_dispatcher;

use crate::config::Config;
use crate::engine;

const SERVICE_NAME: &str = "codex_telegram_bridge";
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

static SERVICE_CONFIG: OnceLock<Config> = OnceLock::new();

define_windows_service!(ffi_service_main, service_main);

pub fn service_name() -> &'static str {
    SERVICE_NAME
}

pub fn install_service(config_path: impl AsRef<Path>) -> Result<PathBuf> {
    let config_path = canonicalize_config_path(config_path)?;
    let manager = native_service_manager().context("failed to open service manager")?;
    manager
        .install(build_install_context(
            std::env::current_exe().context("failed to locate current executable")?,
            config_path.clone(),
        )?)
        .context("failed to install windows service")?;
    Ok(config_path)
}

pub fn uninstall_service() -> Result<()> {
    let manager = native_service_manager().context("failed to open service manager")?;
    manager
        .uninstall(ServiceUninstallCtx {
            label: service_label()?,
        })
        .context("failed to uninstall windows service")?;
    Ok(())
}

pub fn start_installed_service() -> Result<()> {
    let manager = native_service_manager().context("failed to open service manager")?;
    manager
        .start(ServiceStartCtx {
            label: service_label()?,
        })
        .context("failed to start windows service")?;
    Ok(())
}

pub fn stop_installed_service() -> Result<()> {
    let manager = native_service_manager().context("failed to open service manager")?;
    manager
        .stop(ServiceStopCtx {
            label: service_label()?,
        })
        .context("failed to stop windows service")?;
    Ok(())
}

pub fn installed_service_status() -> Result<ManagedServiceStatus> {
    let manager = native_service_manager().context("failed to open service manager")?;
    manager
        .status(ServiceStatusCtx {
            label: service_label()?,
        })
        .context("failed to query windows service status")
}

pub fn format_service_status(status: &ManagedServiceStatus) -> String {
    match status {
        ManagedServiceStatus::NotInstalled => "not_installed".to_owned(),
        ManagedServiceStatus::Running => "running".to_owned(),
        ManagedServiceStatus::Stopped(Some(reason)) => format!("stopped ({reason})"),
        ManagedServiceStatus::Stopped(None) => "stopped".to_owned(),
    }
}

pub fn run_service_mode(config: Config) -> Result<()> {
    SERVICE_CONFIG
        .set(config)
        .map_err(|_| anyhow!("service config already initialized"))?;
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
        .context("failed to start windows service dispatcher")?;
    Ok(())
}

pub fn service_main(_arguments: Vec<OsString>) {
    if let Err(error) = run_service() {
        eprintln!("service failed: {error:#}");
    }
}

fn run_service() -> Result<()> {
    let config = SERVICE_CONFIG
        .get()
        .cloned()
        .ok_or_else(|| anyhow!("service config is not initialized"))?;
    let shutdown = CancellationToken::new();
    let stop_token = shutdown.clone();

    let event_handler = move |control| -> ServiceControlHandlerResult {
        match control {
            ServiceControl::Stop | ServiceControl::Shutdown => {
                stop_token.cancel();
                ServiceControlHandlerResult::NoError
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)
        .context("failed to register windows service handler")?;

    status_handle
        .set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Running,
            controls_accepted: ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })
        .context("failed to report running service state")?;

    let runtime = Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to build tokio runtime for service mode")?;

    let run_result = runtime.block_on(engine::run_with_shutdown(config.clone(), shutdown.clone()));

    shutdown.cancel();
    runtime.shutdown_timeout(Duration::from_secs(config.service.shutdown_grace_sec));

    status_handle
        .set_service_status(ServiceStatus {
            service_type: SERVICE_TYPE,
            current_state: ServiceState::Stopped,
            controls_accepted: ServiceControlAccept::empty(),
            exit_code: ServiceExitCode::Win32(0),
            checkpoint: 0,
            wait_hint: Duration::default(),
            process_id: None,
        })
        .context("failed to report stopped service state")?;

    run_result
}

fn canonicalize_config_path(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    std::fs::canonicalize(path)
        .with_context(|| format!("failed to resolve config path {}", path.display()))
}

fn service_label() -> Result<ServiceLabel> {
    ServiceLabel::from_str(SERVICE_NAME).context("failed to parse windows service label")
}

fn build_install_context(program: PathBuf, config_path: PathBuf) -> Result<ServiceInstallCtx> {
    let working_directory = config_path
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("config path must have a parent directory"))?;

    Ok(ServiceInstallCtx {
        label: service_label()?,
        program,
        args: vec![
            OsString::from("service"),
            OsString::from("run"),
            OsString::from("--config"),
            config_path.into_os_string(),
        ],
        contents: None,
        username: None,
        working_directory: Some(working_directory),
        environment: None,
        autostart: true,
        restart_policy: RestartPolicy::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::{build_install_context, format_service_status, service_name};
    use service_manager::ServiceStatus as ManagedServiceStatus;
    use std::ffi::OsString;
    use std::path::PathBuf;

    #[test]
    fn build_install_context_uses_service_host_and_config_directory() {
        let context = build_install_context(
            PathBuf::from("C:/tools/codex-telegram-bridge.exe"),
            PathBuf::from("C:/workspace/bridge.toml"),
        )
        .expect("install context should build");

        assert_eq!(context.label.to_string(), service_name());
        assert_eq!(
            context.program,
            PathBuf::from("C:/tools/codex-telegram-bridge.exe")
        );
        assert_eq!(
            context.args,
            vec![
                OsString::from("service"),
                OsString::from("run"),
                OsString::from("--config"),
                OsString::from("C:/workspace/bridge.toml"),
            ]
        );
        assert_eq!(
            context.working_directory,
            Some(PathBuf::from("C:/workspace"))
        );
        assert!(context.autostart);
    }

    #[test]
    fn format_service_status_reports_reason_when_present() {
        assert_eq!(
            format_service_status(&ManagedServiceStatus::Stopped(Some("manual".to_owned()))),
            "stopped (manual)"
        );
        assert_eq!(
            format_service_status(&ManagedServiceStatus::Running),
            "running"
        );
        assert_eq!(
            format_service_status(&ManagedServiceStatus::NotInstalled),
            "not_installed"
        );
    }
}

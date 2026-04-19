use std::ffi::OsString;
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
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

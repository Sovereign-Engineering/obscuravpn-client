//! Windows Service Control Manager (SCM) integration for `obscura.exe`.

use std::ffi::{OsString, c_void};
use std::os::windows::io::AsRawHandle;
use std::ptr::null_mut;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;

use obscuravpn_client::logging::LogPersistence;
use tokio::runtime::Handle;
use tokio::sync::watch;
use windows::Win32::Foundation::{HLOCAL, LocalFree};
use windows::Win32::System::Services::{
    QueryServiceDynamicInformation, SERVICE_DYNAMIC_INFORMATION_LEVEL_START_REASON, SERVICE_START_REASON_AUTO, SERVICE_START_REASON_DELAYEDAUTO,
    SERVICE_START_REASON_DEMAND, SERVICE_START_REASON_RESTART_ON_FAILURE, SERVICE_START_REASON_TRIGGER, SERVICE_STATUS_HANDLE,
};
use windows_service::service::{ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType};
use windows_service::service_control_handler::{self, ServiceControlHandlerResult, ServiceStatusHandle};
use windows_service::{define_windows_service, service_dispatcher};

use crate::ServiceArgs;
use crate::service::ScmStartReason;

pub const SERVICE_NAME: &str = "Obscura VPN Service";
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

struct ServiceContext {
    handle: Handle,
    config_dir: String,
    log_persistence: Mutex<Option<LogPersistence>>,
}
static CONTEXT: OnceLock<ServiceContext> = OnceLock::new();

/// Hands control to the SCM dispatcher, which blocks until the service stops
pub fn run(config_dir: String, log_persistence: Option<LogPersistence>) -> Result<(), windows_service::Error> {
    let _ = CONTEXT.set(ServiceContext { handle: Handle::current(), config_dir, log_persistence: Mutex::new(log_persistence) });
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
}

define_windows_service!(ffi_service_main, service_main);

// https://learn.microsoft.com/en-us/windows/win32/services/writing-a-servicemain-function
fn service_main(_arguments: Vec<OsString>) {
    let context = CONTEXT.get().expect("service context set before dispatcher start");
    let log_persistence = context.log_persistence.lock().unwrap().take();
    if let Err(error) = context.handle.block_on(run_service(&context.config_dir, log_persistence)) {
        tracing::error!(message_id = "h3pVw0Qe", ?error, "windows service failed");
    }
}

/// Must exceed the graceful teardown in `service::run` so SCM doesn't kill the process mid-cleanup.
const STOP_WAIT_HINT: Duration = Duration::from_secs(30);

fn status(current_state: ServiceState, controls_accepted: ServiceControlAccept, exit_code: ServiceExitCode, wait_hint: Duration) -> ServiceStatus {
    ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state,
        controls_accepted,
        exit_code,
        checkpoint: 0,
        wait_hint,
        process_id: None,
    }
}

async fn run_service(config_dir: &str, log_persistence: Option<LogPersistence>) -> Result<(), windows_service::Error> {
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let handler_status_handle: Arc<OnceLock<ServiceStatusHandle>> = Arc::new(OnceLock::new());
    let event_handler = {
        let handler_status_handle = handler_status_handle.clone();
        move |control_event| -> ServiceControlHandlerResult {
            match control_event {
                // TODO: https://linear.app/soveng/issue/OBS-3841/windows-should-we-report-vpn-status-on-windows-service-shutdown
                ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
                ServiceControl::Stop | ServiceControl::Shutdown => {
                    let status_handle = handler_status_handle.wait();
                    if let Err(error) = status_handle.set_service_status(status(
                        ServiceState::StopPending,
                        ServiceControlAccept::empty(),
                        ServiceExitCode::Win32(0),
                        STOP_WAIT_HINT,
                    )) {
                        tracing::warn!(message_id = "vB3nRq8Y", ?error, "failed to report stop pending status");
                    }
                    let _ = shutdown_tx.send(true);
                    ServiceControlHandlerResult::NoError
                }
                _ => ServiceControlHandlerResult::NotImplemented,
            }
        }
    };

    // https://learn.microsoft.com/windows/win32/api/winsvc/nf-winsvc-registerservicectrlhandlerexa
    // https://learn.microsoft.com/windows/win32/services/service-status-transitions
    let status_handle = match service_control_handler::register(SERVICE_NAME, event_handler) {
        Ok(status_handle) => status_handle,
        Err(error) => {
            tracing::error!(message_id = "kR7pW2nD", ?error, "failed to register service control handler");
            return Err(error);
        }
    };
    let _ = handler_status_handle.set(status_handle);
    status_handle.set_service_status(status(
        ServiceState::Running,
        ServiceControlAccept::STOP | ServiceControlAccept::SHUTDOWN,
        ServiceExitCode::Win32(0),
        Duration::default(),
    ))?;

    let scm_start_reason = ScmStartReason::query(status_handle);
    let args = ServiceArgs { config_dir: config_dir.to_owned(), runtime_dir: None };
    let mut exit_code = ServiceExitCode::Win32(0);
    if let Err(error) = crate::service::run(args, log_persistence, Some(shutdown_rx), Some(scm_start_reason)).await {
        tracing::error!(message_id = "Yb2mK9rL", %error, "service exited with error");
        exit_code = ServiceExitCode::ServiceSpecific(1);
    }
    tracing::info!(message_id = "iPqn0S7F", "scm service run finished; exiting");
    obscuravpn_client::logging::flush_and_stop_persisted_log();
    status_handle.set_service_status(status(
        ServiceState::Stopped,
        ServiceControlAccept::empty(),
        exit_code,
        Duration::default(),
    ))?;
    Ok(())
}

impl ScmStartReason {
    pub fn query(status_handle: ServiceStatusHandle) -> Self {
        let mut info: *mut c_void = null_mut();
        // SAFETY: `status_handle` was registered for this service and stays valid for the lifetime of the process.
        if let Err(error) = unsafe {
            QueryServiceDynamicInformation(
                SERVICE_STATUS_HANDLE(status_handle.as_raw_handle()),
                SERVICE_DYNAMIC_INFORMATION_LEVEL_START_REASON,
                &mut info,
            )
        } {
            tracing::error!(message_id = "pQ4vNs8K", ?error, "failed to query service start reason");
            return Self::Unknown;
        }
        if info.is_null() {
            tracing::error!(message_id = "mB7cWd2J", "service start reason query returned no data");
            return Self::Unknown;
        }
        // SAFETY: for SERVICE_DYNAMIC_INFORMATION_LEVEL_START_REASON the buffer holds a u32 of SERVICE_START_REASON_* flags, owned by us until freed with LocalFree.
        let raw_start_reason = unsafe { *info.cast::<u32>() };
        // SAFETY: `info` was allocated by `QueryServiceDynamicInformation` and is not used after this.
        let _ = unsafe { LocalFree(Some(HLOCAL(info))) };
        let start_reason = match raw_start_reason {
            SERVICE_START_REASON_AUTO => Self::Auto,
            SERVICE_START_REASON_DELAYEDAUTO => Self::DelayedAuto,
            SERVICE_START_REASON_DEMAND => Self::Demand,
            SERVICE_START_REASON_RESTART_ON_FAILURE => Self::FailureRecovery,
            SERVICE_START_REASON_TRIGGER => Self::Trigger,
            _ => Self::Unknown,
        };
        tracing::info!(message_id = "rF3xKt9L", raw_start_reason, ?start_reason, "queried service start reason");
        start_reason
    }
}

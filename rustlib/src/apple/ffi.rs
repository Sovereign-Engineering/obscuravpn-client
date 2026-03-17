use std::sync::{Arc, OnceLock};
use tokio::runtime::Runtime;

use super::os_impl::AppleOsImpl;
use crate::config::KeychainSetSecretKeyFn;
use crate::ffi_helpers::*;
use crate::logging::LogPersistence;
use crate::manager::Manager;
use crate::manager_cmd::ManagerCmd;
use crate::manager_cmd::ManagerCmdErrorCode;
use crate::net::NetworkInterface;
use crate::positive_u31::PositiveU31;

/// cbindgen:ignore
static APPLE_LOG_INIT: std::sync::Once = std::sync::Once::new();

/// To view logs info, or debug logs with `log` tool you must pass `--level info|debug`.
/// To filter logs at the rust level you can set the `RUST_LOG` environment variable.
///
/// On iOS, returns the pointer for the drop guard that flushes log writes.
///
/// SAFETY:
/// - there is no other global function of this name
#[unsafe(no_mangle)]
pub extern "C" fn initialize_apple_system_logging(log_dir: FfiStr) -> *mut LogPersistence {
    let mut log_persistence: Option<LogPersistence> = None;
    APPLE_LOG_INIT.call_once(|| {
        log_persistence = crate::logging::init(
            tracing_oslog::OsLogger::new("net.obscura.rust-apple", "default"),
            cfg!(target_os = "ios").then(|| log_dir.as_str().as_ref()),
        )
    });
    log_persistence.map(Box::new).map(Box::into_raw).unwrap_or(std::ptr::null_mut())
}

pub struct Global {
    manager: Arc<Manager>,
    os_impl: Arc<AppleOsImpl>,
    runtime: Runtime,
}

/// cbindgen:ignore
static GLOBAL: OnceLock<Global> = OnceLock::new();

/// SAFETY:
/// - `log_persistence` must be a pointer returned by `initialize_apple_system_logging`
/// - there is no other global function of this name
#[unsafe(no_mangle)]
pub unsafe extern "C" fn initialize(
    config_dir: FfiStr,
    user_agent: FfiStr,
    keychain_wg_secret_key: FfiBytes,
    receive_cb: extern "C" fn(FfiBytes),
    set_network_config_cb: super::os_impl::SetNetworkConfigCb,
    keychain_set_wg_secret_key: extern "C" fn(FfiBytes) -> bool,
    log_persistence: *mut LogPersistence,
) -> *const Global {
    tracing::info!(message_id = "PRXlxa85", "starting ffi initialization");
    let mut first_init = false;
    let global: &'static Global = GLOBAL.get_or_init(|| {
        rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .expect("Failed to install aws-lc crypto provider");

        let runtime = Runtime::new().expect("Failed to create tokio runtime");
        let _runtime_guard = runtime.enter();

        let config_dir = config_dir.to_string().into();
        let user_agent = user_agent.to_string();
        let keychain_wg_sk = Some(keychain_wg_secret_key.to_vec()).filter(|v| !v.is_empty());
        let keychain_set_wg_secret_key: KeychainSetSecretKeyFn = Box::new(move |sk: &[u8; 32]| keychain_set_wg_secret_key(sk.ffi()));
        let log_persistence = std::ptr::NonNull::new(log_persistence).map(|log_persistence|
            // SAFETY:
            // - `log_persistence` was checked to be non-null
            // - Caller guarantees that `log_persistence` originates from a
            //   matching `into_raw` call
            *unsafe { Box::from_raw(log_persistence.as_ptr()) });
        let os_impl = Arc::new(AppleOsImpl::new(receive_cb, set_network_config_cb));
        match Manager::new(
            config_dir,
            keychain_wg_sk.as_deref(),
            user_agent,
            os_impl.clone(),
            Some(keychain_set_wg_secret_key),
            log_persistence,
            true, // persistent tunnel activation must be handled by the on-demand OS feature on Apple platforms
        ) {
            Ok(manager) => {
                first_init = true;
                tracing::info!(message_id = "Y6cNkZXW", "ffi initialized");
                Global { manager, os_impl, runtime }
            }
            Err(err) => panic!("ffi initialization failed: could not load config: {}", err),
        }
    });
    if !first_init {
        tracing::error!(message_id = "GQRW1s5V", "ffi was already initialized")
    }
    std::ptr::from_ref(global)
}

#[allow(dead_code)]
#[repr(u8)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

#[unsafe(no_mangle)]
pub extern "C" fn forward_log(level: LogLevel, message: FfiStr, file_id: FfiStr, function: FfiStr, line: isize) {
    let message = message.as_str();
    let file_id = file_id.as_str();
    let function = function.as_str();
    // https://github.com/tokio-rs/tracing/issues/372
    match level {
        LogLevel::Trace => tracing::event!(tracing::Level::TRACE, message, file_id, function, line),
        LogLevel::Debug => tracing::event!(tracing::Level::DEBUG, message, file_id, function, line),
        LogLevel::Info => tracing::event!(tracing::Level::INFO, message, file_id, function, line),
        LogLevel::Warn => tracing::event!(tracing::Level::WARN, message, file_id, function, line),
        LogLevel::Error => tracing::event!(tracing::Level::ERROR, message, file_id, function, line),
    }
}

/// SAFETY:
/// - `global` must be a pointer returned by `initialize`
/// - there is no other global function of this name
#[unsafe(no_mangle)]
pub unsafe extern "C" fn send_packet(global: *const Global, packet: FfiBytes) {
    // SAFETY: `global` was created by a matching call to `std::ptr::from_ref`
    let global = unsafe { &*global };
    global.os_impl.send_packet(packet.as_slice());
}

/// Set the network interface to use by index and name.
///
/// Index 0 means no interface (do not try connecting).
/// Name is ignored if index is 0.
///
/// SAFETY:
/// - `global` must be a pointer returned by `initialize`
/// - there is no other global function of this name
#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_network_interface(global: *const Global, index: u32, name: FfiStr) {
    // SAFETY: `global` was created by a matching call to `std::ptr::from_ref`
    let global = unsafe { &*global };
    let network_interface = match index {
        0 => None,
        index => PositiveU31::try_from(index)
            .map_err(|_| tracing::error!(message_id = "Y8aRUbjp", index, "network interface index out of range"))
            .ok(),
    }
    .map(|index| NetworkInterface { index, name: name.as_str().to_string() });
    global.os_impl.set_network_interface(network_interface);
}

/// Call after wake.
///
/// SAFETY:
/// - `global` must be a pointer returned by `initialize`
/// - there is no other global function of this name
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wake(global: *const Global) {
    // SAFETY: `global` was created by a matching call to `std::ptr::from_ref`
    let global = unsafe { &*global };
    global.manager.wake();
}

/// SAFETY:
/// - `global` must be a pointer returned by `initialize`
/// - there is no other global function of this name
#[unsafe(no_mangle)]
pub unsafe extern "C" fn json_ffi_cmd(
    global: *const Global,
    context: usize,
    json_cmd: FfiBytes,
    cb: extern "C" fn(context: usize, json_ret: FfiStr, json_err: FfiStr),
) {
    // SAFETY: `global` was created by a matching call to `std::ptr::from_ref`
    let global = unsafe { &*global };
    let json_cmd = json_cmd.to_vec();

    global.runtime.spawn(async move {
        let manager = &global.manager;

        let json_result: Result<String, ManagerCmdErrorCode> = async move {
            let ok = ManagerCmd::from_json(&json_cmd)?.run(manager).await?;
            serde_json::to_string(&ok).map_err(|error| {
                tracing::error!(message_id = "TFqFKASM", ?error, "could not serialize successful json cmd result: {error}");
                ManagerCmdErrorCode::Other
            })
        }
        .await;

        match json_result {
            Ok(ok) => cb(context, ok.ffi_str(), "".ffi_str()),
            Err(err) => cb(context, "".ffi_str(), err.as_static_str().ffi_str()),
        }
    });
}

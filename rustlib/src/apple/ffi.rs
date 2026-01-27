use std::ffi::c_void;
use std::sync::{Arc, LazyLock, OnceLock};
use tokio::runtime::Runtime;

use crate::config::KeychainSetSecretKeyFn;
use crate::ffi_helpers::*;
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
pub extern "C" fn initialize_apple_system_logging(log_dir: FfiStr) -> *mut c_void {
    let mut guard_ptr = std::ptr::null_mut();
    APPLE_LOG_INIT.call_once(|| {
        if let Some(guard) = crate::logging::init(
            tracing_oslog::OsLogger::new("net.obscura.rust-apple", "default"),
            cfg!(target_os = "ios").then(|| log_dir.as_str().as_ref()),
        ) {
            guard_ptr = Box::into_raw(guard) as *mut _;
        };
    });
    guard_ptr
}

/// cbindgen:ignore
static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| Runtime::new().unwrap());
static GLOBAL: OnceLock<Arc<Manager>> = OnceLock::new();

fn global_manager() -> Arc<Manager> {
    GLOBAL.get().expect("ffi global manager not initialized").clone()
}

/// SAFETY:
/// - `log_persistence` must be a pointer returned by `initialize_apple_system_logging`
/// - there is no other global function of this name
/// - (TODO)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn initialize(
    config_dir: FfiStr,
    user_agent: FfiStr,
    keychain_wg_secret_key: FfiBytes,
    receive_cb: extern "C" fn(FfiBytes),
    keychain_set_wg_secret_key: extern "C" fn(FfiBytes) -> bool,
    log_persistence: *mut c_void,
) {
    let mut first_init = false;
    GLOBAL.get_or_init(|| {
        rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .expect("Failed to install aws-lc crypto provider");

        let config_dir = config_dir.to_string().into();
        let user_agent = user_agent.to_string();
        let keychain_wg_sk = Some(keychain_wg_secret_key.to_vec()).filter(|v| !v.is_empty());
        let keychain_set_wg_secret_key: KeychainSetSecretKeyFn = Box::new(move |sk: &[u8; 32]| keychain_set_wg_secret_key(sk.ffi()));
        let log_persistence = std::ptr::NonNull::new(log_persistence).map(|log_persistence|
            // SAFETY:
            // - `log_persistence` was checked to be non-null
            // - Caller guarantees that `log_persistence` originates from a
            //   matching `into_raw` call
            unsafe { Box::from_raw(log_persistence.as_ptr() as _) });
        match Manager::new(
            config_dir,
            keychain_wg_sk.as_deref(),
            user_agent,
            RUNTIME.handle().clone(),
            receive_cb,
            Some(keychain_set_wg_secret_key),
            log_persistence,
        ) {
            Ok(c) => {
                first_init = true;
                tracing::info!("ffi initialized");
                c
            }
            Err(err) => panic!("ffi initialization failed: could not load config: {}", err),
        }
    });
    if !first_init {
        tracing::info!("ffi already initialized")
    }
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
/// - there is no other global function of this name
#[unsafe(no_mangle)]
pub unsafe extern "C" fn send_packet(packet: FfiBytes) {
    let packet = packet.to_vec();
    global_manager().send_packet(&packet);
}

/// Set the network interface to use by index and name.
///
/// Index 0 means no interface (do not try connecting).
/// Name is ignored if index is 0.
///
/// SAFETY:
/// - there is no other global function of this name
#[unsafe(no_mangle)]
pub unsafe extern "C" fn set_network_interface(index: u32, name: FfiStr) {
    let network_interface = match index {
        0 => None,
        index => PositiveU31::try_from(index)
            .map_err(|_| tracing::error!(message_id = "Y8aRUbjp", index, "network interface index out of range"))
            .ok(),
    }
    .map(|index| NetworkInterface { index, name: name.as_str().to_string() });
    global_manager().set_network_interface(network_interface);
}

/// Call after wake.
///
/// SAFETY:
/// - there is no other global function of this name
#[unsafe(no_mangle)]
pub unsafe extern "C" fn wake() {
    global_manager().wake();
}

/// SAFETY:
/// - there is no other global function of this name
#[unsafe(no_mangle)]
pub unsafe extern "C" fn json_ffi_cmd(context: usize, json_cmd: FfiBytes, cb: extern "C" fn(context: usize, json_ret: FfiStr, json_err: FfiStr)) {
    let json_cmd = json_cmd.to_vec();

    RUNTIME.spawn(async move {
        let manager = global_manager();

        let json_result: Result<String, ManagerCmdErrorCode> = async move {
            let ok = ManagerCmd::from_json(&json_cmd)?.run(&manager).await?;
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

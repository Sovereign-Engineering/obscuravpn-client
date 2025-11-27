use std::ffi::c_void;
use std::num::NonZeroU32;
use std::sync::{Arc, LazyLock, OnceLock};
use tokio::runtime::Runtime;

use crate::config::KeychainSetSecretKeyFn;
use crate::ffi_helpers::*;
use crate::manager::Manager;
use crate::manager_cmd::ManagerCmd;
use crate::manager_cmd::ManagerCmdErrorCode;
use crate::net::NetworkInterface;

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
    use tracing_oslog::OsLogger;
    use tracing_subscriber::{
        Layer as _,
        filter::{EnvFilter, LevelFilter},
        layer::SubscriberExt as _,
        registry,
    };
    // `EnvFilter` doesn't impl `Clone`
    fn filter() -> EnvFilter {
        EnvFilter::from_default_env().add_directive(LevelFilter::INFO.into())
    }
    #[cfg(not(target_os = "ios"))]
    let _ = log_dir;
    #[cfg_attr(not(target_os = "ios"), allow(unused_mut))]
    let mut guard_ptr = std::ptr::null_mut();
    APPLE_LOG_INIT.call_once(|| {
        let oslog_layer = OsLogger::new("net.obscura.rust-apple", "default").with_filter(filter());
        let registry = registry().with(oslog_layer);
        #[cfg(not(target_os = "ios"))]
        tracing::subscriber::set_global_default(registry).expect("failed to set global subscriber");
        #[cfg(target_os = "ios")]
        match super::ios::build_log_roller(&log_dir) {
            Ok(roller) => {
                use tracing_appender::non_blocking::NonBlocking;
                let (writer, guard) = NonBlocking::new(roller);
                guard_ptr = Box::into_raw(Box::new(guard)) as _;
                let fs_layer = tracing_subscriber::fmt::Layer::default().json().with_writer(writer).with_filter(filter());
                tracing::subscriber::set_global_default(registry.with(fs_layer)).expect("failed to set global subscriber");
            }
            Err(error) => {
                tracing::subscriber::set_global_default(registry).expect("failed to set global subscriber");
                tracing::error!(?error, "failed to initialize log persistence");
            }
        }
        tracing::info!("logging initialized");
        std::panic::set_hook(Box::new(|info| {
            let backtrace = std::backtrace::Backtrace::force_capture();
            tracing::error!("panic: {}\n{:#}", info, backtrace);
        }));
        tracing::info!("panic logging hook set");
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
/// - `log_flush_guard` must be a pointer returned by `initialize_apple_system_logging`
/// - there is no other global function of this name
/// - (TODO)
#[unsafe(no_mangle)]
pub unsafe extern "C" fn initialize(
    config_dir: FfiStr,
    user_agent: FfiStr,
    keychain_wg_secret_key: FfiBytes,
    receive_cb: extern "C" fn(FfiBytes),
    keychain_set_wg_secret_key: extern "C" fn(FfiBytes) -> bool,
    log_flush_guard: *mut c_void,
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
        let log_flush_guard = std::ptr::NonNull::new(log_flush_guard).map(|log_flush_guard|
            // SAFETY:
            // - `log_flush_guard` was checked to be non-null
            // - Caller guarantees that `log_flush_guard` originates from a
            //   matching `into_raw` call
            unsafe { Box::from_raw(log_flush_guard.as_ptr() as _) });
        match Manager::new(
            config_dir,
            keychain_wg_sk.as_deref(),
            user_agent,
            &RUNTIME,
            receive_cb,
            Some(keychain_set_wg_secret_key),
            log_flush_guard,
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
    let network_interface = NonZeroU32::new(index).map(|index| NetworkInterface { index, name: name.as_str().to_string() });
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

    let hash = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, &json_cmd);

    let cmd: ManagerCmd = match serde_json::from_slice(&json_cmd) {
        Ok(cmd) => cmd,
        Err(error) => {
            tracing::error!(
                ?error,
                cmd =? String::from_utf8_lossy(&json_cmd),
                hash =? hash,
                message_id = "ahsh9Aec",
                "could not decode json command: {error}",
            );
            let err: &'static str = ManagerCmdErrorCode::Other.into();
            cb(context, "".ffi_str(), err.ffi_str());
            return;
        }
    };

    tracing::info!(
        cmd = format!("{:#?}", cmd),
        hash =? hash,
        message_id = "JumahFi5",
        "received json ffi cmd",
    );

    RUNTIME.spawn(async move {
        let manager = global_manager();

        let result = cmd.run(&manager).await;

        tracing::info!(
            result = format!("{:#?}", result),
            hash =? hash,
            message_id = "eed0Oogi",
            "finished json ffi cmd",
        );

        let json_result = result.and_then(|ok| {
            serde_json::to_string(&ok).map_err(|err| {
                tracing::error!(?err, "could not serialize successful json cmd result: {err}");
                ManagerCmdErrorCode::Other
            })
        });

        match json_result {
            Ok(ok) => cb(context, ok.ffi_str(), "".ffi_str()),
            Err(err) => cb(context, "".ffi_str(), <&'static str>::from(err).ffi_str()),
        }
    });
}

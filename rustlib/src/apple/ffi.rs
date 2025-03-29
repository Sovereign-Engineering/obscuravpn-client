use std::sync::{Arc, LazyLock, OnceLock};
use tokio::runtime::Runtime;

use crate::ffi_helpers::*;
use crate::manager::Manager;
use crate::manager_cmd::ManagerCmd;
use crate::manager_cmd::ManagerCmdErrorCode;

/// cbindgen:ignore
static MACOS_LOG_INIT: std::sync::Once = std::sync::Once::new();

#[no_mangle]
/// To view logs info, or debug logs with `log` tool you must pass `--level info|debug`.
/// To filter logs at the rust level you can set the `RUST_LOG` environment variable.
pub extern "C" fn initialize_macos_system_logging() {
    use tracing_oslog::OsLogger;
    use tracing_subscriber::{
        filter::{EnvFilter, LevelFilter},
        layer::SubscriberExt,
        registry, Layer,
    };

    MACOS_LOG_INIT.call_once(|| {
        let filter = EnvFilter::from_default_env().add_directive(LevelFilter::INFO.into());
        let collector = registry().with(OsLogger::new("net.obscura.rust-apple", "default").with_filter(filter));

        tracing::subscriber::set_global_default(collector).expect("failed to set global subscriber");
        tracing::info!("logging initialized");

        std::panic::set_hook(Box::new(|info| {
            let backtrace = std::backtrace::Backtrace::force_capture();
            tracing::error!("panic: {}\n{:#}", info, backtrace);
        }));
        tracing::info!("panic logging hook set");
    });
}

/// cbindgen:ignore
static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| Runtime::new().unwrap());
static GLOBAL: OnceLock<Arc<Manager>> = OnceLock::new();

fn global_manager() -> Arc<Manager> {
    GLOBAL.get().expect("ffi global manager not initialized").clone()
}

#[no_mangle]
pub unsafe extern "C" fn initialize(config_dir: FfiStr, old_config_dir: FfiStr, user_agent: FfiStr, receive_cb: extern "C" fn(FfiBytes)) {
    let mut first_init = false;
    GLOBAL.get_or_init(|| {
        let config_dir = config_dir.to_string().into();
        let old_config_dir = old_config_dir.to_string().into();
        let user_agent = user_agent.to_string();
        match Manager::new(config_dir, old_config_dir, user_agent, &RUNTIME, receive_cb) {
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

#[no_mangle]
pub unsafe extern "C" fn send_packet(packet: FfiBytes) {
    let packet = packet.to_vec();
    global_manager().send_packet(&packet);
}

#[no_mangle]
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

        let json_result = result.and_then(|ok| match serde_json::to_string(&ok) {
            Ok(json_ok) => Ok(json_ok),
            Err(err) => {
                tracing::error!(?err, "could not serialize successful json cmd result: {err}");
                Err(ManagerCmdErrorCode::Other)
            }
        });

        match json_result {
            Ok(ok) => cb(context, ok.ffi_str(), "".ffi_str()),
            Err(err) => {
                let err: &'static str = err.into();
                cb(context, "".ffi_str(), err.ffi_str())
            }
        }
    });
}

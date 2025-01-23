use std::sync::{Arc, LazyLock, OnceLock};
use tokio::runtime::Runtime;

use crate::errors::ConnectErrorCode;
use crate::ffi_helpers::*;
use crate::manager::Manager;
use crate::manager::TunnelArgs;
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
pub unsafe extern "C" fn initialize(config_dir: FfiStr, old_config_dir: FfiStr, user_agent: FfiStr) {
    let mut first_init = false;
    GLOBAL.get_or_init(|| {
        let config_dir = config_dir.to_string().into();
        let old_config_dir = old_config_dir.to_string().into();
        let user_agent = user_agent.to_string();
        match Manager::new(config_dir, old_config_dir, user_agent) {
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
pub unsafe extern "C" fn start_tunnel(
    context: usize,
    json_tunnel_args: FfiStr,
    receive_cb: extern "C" fn(FfiBytes),
    network_config_cb: extern "C" fn(FfiBytes),
    tunnel_status_cb: extern "C" fn(isConnected: bool),
    cb: extern "C" fn(context: usize, network_config: FfiBytes, err: FfiStr),
) {
    let json_tunnel_args = json_tunnel_args.to_string();
    tracing::info!("start_tunnel args: {}", &json_tunnel_args);
    let tunnel_args: TunnelArgs = match serde_json::from_str(&json_tunnel_args) {
        Ok(cmd) => cmd,
        Err(err) => {
            tracing::error!(?err, "could not decode json tunnel args: {err}");
            let err: &'static str = ConnectErrorCode::Other.into();
            cb(context, [].ffi(), err.ffi_str());
            return;
        }
    };
    RUNTIME.spawn(async move {
        match global_manager().start(tunnel_args, receive_cb, network_config_cb, tunnel_status_cb).await {
            Ok(network_config) => {
                let network_config_json = serde_json::to_vec(&network_config).unwrap();
                cb(context, network_config_json.ffi(), "".ffi_str())
            }
            Err(err) => {
                let err: &'static str = err.into();
                cb(context, [].ffi(), err.ffi_str())
            }
        }
    });
}

#[no_mangle]
pub unsafe extern "C" fn send_packet(packet: FfiBytes) {
    let packet = packet.to_vec();
    global_manager().send_packet(&packet);
}

#[no_mangle]
pub extern "C" fn stop_tunnel() {
    global_manager().stop();
}

#[no_mangle]
pub unsafe extern "C" fn json_ffi_cmd(context: usize, json_cmd: FfiStr, cb: extern "C" fn(context: usize, json_ret: FfiStr, json_err: FfiStr)) {
    let json_cmd = json_cmd.to_string();
    tracing::info!("received json ffi cmd: {}", &json_cmd);
    let cmd: ManagerCmd = match serde_json::from_str(&json_cmd) {
        Ok(cmd) => cmd,
        Err(err) => {
            tracing::error!(?err, "could not decode json command: {err}");
            let err: &'static str = ManagerCmdErrorCode::Other.into();
            cb(context, "".ffi_str(), err.ffi_str());
            return;
        }
    };
    RUNTIME.spawn(async move {
        let manager = global_manager();
        let json_result: Result<String, ManagerCmdErrorCode> = cmd.run(&manager).await.and_then(|ok| match serde_json::to_string_pretty(&ok) {
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

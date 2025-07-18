use std::sync::{Arc, LazyLock, OnceLock};
use tokio::runtime::Runtime;

use crate::config::KeychainSetSecretKeyFn;
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
pub unsafe extern "C" fn initialize(
    config_dir: FfiStr,
    user_agent: FfiStr,
    keychain_wg_secret_key: FfiBytes,
    receive_cb: extern "C" fn(FfiBytes),
    keychain_set_wg_secret_key: extern "C" fn(FfiBytes) -> bool,
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
        match Manager::new(
            config_dir,
            keychain_wg_sk.as_deref(),
            user_agent,
            &RUNTIME,
            receive_cb,
            Some(keychain_set_wg_secret_key),
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

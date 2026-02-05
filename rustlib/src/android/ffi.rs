use super::{
    MANAGER, RUNTIME,
    future::signal_json_ffi_future,
    get_manager,
    tunnel::Tun,
    util::{Utf8JavaStr, throw_runtime_exception},
};
use crate::{ffi_helpers::FfiBytes, manager::Manager, manager_cmd::ManagerCmd, net::NetworkInterface, positive_u31::PositiveU31};
use anyhow::Context as _;
use jni::{
    JNIEnv, JavaVM,
    objects::{JClass, JObject, JString},
    sys::jint,
};
use nix::net::if_::if_indextoname;
use std::{
    ffi::c_void,
    os::fd::{FromRawFd as _, OwnedFd},
    sync::{Arc, Mutex},
};

const RUST_LOG_DIR_NAME: &str = "rust-log";

static TUN: Mutex<Option<Tun>> = Mutex::new(None);

/// cbindgen:ignore
extern "C" fn receive_cb(ffi_bytes: FfiBytes) {
    if let Some(tun) = &*TUN.lock().unwrap() {
        tun.write(&ffi_bytes.as_slice());
    }
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub extern "C" fn JNI_OnLoad(vm: *mut jni::sys::JavaVM, _reserved: *mut c_void) -> jint {
    // `JNI_OnLoad` is called by the Java VM automatically, so we can get away
    // with calling `expect` and making other strong assumptions.
    // SAFETY: `vm` is the current Java VM
    let vm = unsafe { JavaVM::from_raw(vm) }.expect("`JNI_OnLoad` called with null VM pointer");
    let mut env = vm.get_env().expect("`JNI_OnLoad` called from detached thread");
    // Looking up app-specific Java classes from native threads isn't possible,
    // so we take advantage of the fact that `JNI_OnLoad` is called from a Java
    // thread to cache all the app-specific classes we need.
    // https://developer.android.com/ndk/guides/jni-tips#faq:-why-didnt-findclass-find-my-class
    super::class_cache::init(&mut env);
    jni::sys::JNI_VERSION_1_6
}

fn initialize(env: &mut JNIEnv, j_config_dir: &JString, j_user_agent: &JString) -> anyhow::Result<Arc<Manager>> {
    let config_dir = Utf8JavaStr::new(env, j_config_dir, "j_config_dir")?;
    let user_agent = Utf8JavaStr::new(env, j_user_agent, "j_user_agent")?;
    let log_dir = config_dir.as_path().join(RUST_LOG_DIR_NAME);
    let log_persistence = crate::logging::init(tracing_android::layer("ObscuraNative")?, Some(&log_dir));
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .map_err(|_| anyhow::format_err!("failed to install crypto provider"))?;
    Manager::new(
        config_dir.as_path().into(),
        None, // TODO: https://linear.app/soveng/issue/OBS-2699/android-keychain-equivalent
        user_agent.as_str().into(),
        RUNTIME.handle().clone(),
        receive_cb,
        None, // TODO: https://linear.app/soveng/issue/OBS-2699/android-keychain-equivalent
        log_persistence,
        true,
    )
    .map_err(Into::into)
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_initialize(
    mut env: JNIEnv,
    _: JClass,
    j_config_dir: JString,
    j_user_agent: JString,
) {
    let mut first_init = false;
    MANAGER.get_or_init(|| {
        // We can remove this panic once `get_or_try_init` is stable:
        // https://github.com/rust-lang/rust/issues/109737
        let manager = initialize(&mut env, &j_config_dir, &j_user_agent).expect("`initialize` failed");
        first_init = true;
        manager
    });
    if !first_init {
        throw_runtime_exception(&mut env, "manager already initialized");
    }
}

// TODO: If we fail to signal the future for any reason, then the app will wait
// forever! It's not possible to make this infallible, so we need timeouts.
// https://linear.app/soveng/issue/OBS-2643/android-command-timeout-retries
fn json_ffi(env: &mut JNIEnv, j_json_cmd: &JString, j_future: &JObject) -> anyhow::Result<()> {
    let json_cmd = Utf8JavaStr::new(env, j_json_cmd, "j_json_cmd")?;
    let cmd = serde_json::from_str::<ManagerCmd>(json_cmd.as_str())?;
    // This extends the Java object's lifetime until dropped.
    let j_future = env.new_global_ref(&j_future)?;
    let manager = get_manager()?;
    let jvm = env.get_java_vm()?;
    RUNTIME.spawn(async move {
        let manager = manager.clone();
        let result = cmd.run(&manager).await;
        // This attaches the current thread to the JVM for the entire life of
        // the thread, which is significantly more performant than
        // attaching/detaching on each use. This will be a no-op if already
        // attached.
        //
        // Since it's attached as a "daemon thread", the life of this thread
        // won't extend the life of the JVM.
        match jvm.attach_current_thread_as_daemon() {
            Ok(mut env) => {
                if let Err(error) = signal_json_ffi_future(&mut env, j_future.as_obj(), result) {
                    tracing::error!(message_id = "OY0SMEhn", ?error, "failed to signal Java future");
                }
            }
            Err(error) => {
                tracing::error!(message_id = "Wg0053Pz", ?error, "failed to attach thread to JVM");
                // We can't interact with the JVM to throw an exception or
                // call methods on the Java future, so we have to give up.
            }
        };
    });
    Ok(())
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_jsonFfi(mut env: JNIEnv, _: JClass, j_json_cmd: JString, j_future: JObject) {
    if let Err(error) = json_ffi(&mut env, &j_json_cmd, &j_future) {
        tracing::error!(message_id = "jmx2DBFz", ?error, "`json_ffi` failed");
        throw_runtime_exception(&mut env, error);
    }
}

fn set_network_interface_index(j_index: jint) -> anyhow::Result<()> {
    let manager = get_manager()?;
    let network_interface = (j_index > 0)
        .then(|| -> anyhow::Result<_> {
            let index = u32::try_from(j_index)
                .and_then(PositiveU31::try_from)
                .context("network interface index wasn't a positive u32")?;
            let name = if_indextoname(index.into())
                .context("failed to get network interface name for index")?
                .into_string()
                .context("failed to convert network interface name to string")?;
            Ok(NetworkInterface { name, index })
        })
        .transpose()?;
    manager.set_network_interface(network_interface);
    Ok(())
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_setNetworkInterfaceIndex(mut env: JNIEnv, _: JClass, j_index: jint) {
    if let Err(error) = set_network_interface_index(j_index) {
        tracing::error!(message_id = "TnqHMA9u", ?error, "`set_network_interface_index` failed");
        throw_runtime_exception(&mut env, error);
    }
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_setTun(mut env: JNIEnv, _: JClass, j_fd: jint) {
    // SAFETY:
    // - `detachFd` surrenders ownership of the FD on the Kotlin side
    // - No cleanup required besides `close`
    let fd = (j_fd >= 0).then(|| unsafe { OwnedFd::from_raw_fd(j_fd) });
    match fd {
        Some(fd) => match Tun::spawn(fd) {
            Ok(tun) => *TUN.lock().unwrap() = Some(tun),
            Err(error) => {
                tracing::error!(message_id = "VjGxw5uw", ?error, "failed to spawn tun reader");
                throw_runtime_exception(&mut env, error);
            }
        },
        None => *TUN.lock().unwrap() = None,
    }
}

// We'd need to use `getStackTrace` to get more information than this, but that
// seems relatively expensive, has a fiddly API, and still isn't exactly what we
// want (i.e. line numbers are for `return` statements).
fn forward_log(
    env: &mut JNIEnv,
    j_level: jint,
    j_tag: &JString,
    j_message: &JString,
    j_message_id: &JString,
    j_throwable_string: &JString,
) -> anyhow::Result<()> {
    let tag = Utf8JavaStr::new(env, j_tag, "j_tag")?;
    let tag = tag.as_str();
    let message = Utf8JavaStr::new(env, j_message, "j_message")?;
    let message = message.as_str();
    let message_id = Utf8JavaStr::new(env, j_message_id, "j_message_id")?;
    let message_id = message_id.as_str();
    let throwable_string = Utf8JavaStr::from_nullable(env, j_throwable_string, "j_throwable_string")?;
    let throwable_string = throwable_string.as_ref().map(Utf8JavaStr::as_str);
    // https://github.com/tokio-rs/tracing/issues/372
    match j_level {
        0 => tracing::event!(target: "java", tracing::Level::TRACE, message_id, tag, throwable_string, message),
        1 => tracing::event!(target: "java", tracing::Level::DEBUG, message_id, tag, throwable_string, message),
        2 => tracing::event!(target: "java", tracing::Level::INFO, message_id, tag, throwable_string, message),
        3 => tracing::event!(target: "java", tracing::Level::WARN, message_id, tag, throwable_string, message),
        4 => tracing::event!(target: "java", tracing::Level::ERROR, message_id, tag, throwable_string, message),
        _ => anyhow::bail!("invalid log level: {j_level}"),
    }
    Ok(())
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_forwardLog(
    mut env: JNIEnv,
    _: JClass,
    j_level: jint,
    j_tag: JString,
    j_message: JString,
    j_message_id: JString,
    j_throwable_string: JString,
) {
    if let Err(error) = forward_log(&mut env, j_level, &j_tag, &j_message, &j_message_id, &j_throwable_string) {
        tracing::error!(message_id = "Cgb1qGM7", ?error, "failed to forward Java logging");
    }
}

use super::{
    class_cache::ClassCache,
    future::signal_json_ffi_future,
    os_impl::AndroidOsImpl,
    util::{Utf8JavaStr, throw_runtime_exception},
};
use crate::{manager::Manager, manager_cmd::ManagerCmd, net::NetworkInterface, positive_u31::PositiveU31};
use anyhow::Context as _;
use jni::{
    JNIEnv, JavaVM,
    objects::{JClass, JObject, JString, JValue},
    sys::{jint, jobject},
};
use std::{
    ffi::c_void,
    os::fd::{FromRawFd as _, OwnedFd},
    sync::Arc,
    sync::OnceLock,
};
use tokio::runtime::Runtime;

pub struct Global {
    pub manager: Arc<Manager>,
    pub os_impl: Arc<AndroidOsImpl>,
    pub class_cache: Arc<ClassCache>,
    pub runtime: Runtime,
}

static GLOBAL: OnceLock<Global> = OnceLock::new();

/// Get global from handle.
///
/// Note: The handle is not actually used, just proof of construction. We rely on the Java type of FFI functions to enforce its existence.
fn global_from_handle(_handle: &JObject) -> &'static Global {
    GLOBAL.get().expect("global not initialized")
}

const RUST_LOG_DIR_NAME: &str = "rust-log";

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub extern "C" fn JNI_OnLoad(_vm: *mut jni::sys::JavaVM, _reserved: *mut c_void) -> jint {
    jni::sys::JNI_VERSION_1_6
}

fn initialize(env: &mut JNIEnv, j_config_dir: &JString, j_user_agent: &JString, class_cache: Arc<ClassCache>) -> anyhow::Result<Global> {
    let runtime = Runtime::new().expect("Failed to create tokio runtime");
    let _runtime_guard = runtime.enter();
    let config_dir = Utf8JavaStr::new(env, j_config_dir, "j_config_dir", "INsGbyhM")?;
    let user_agent = Utf8JavaStr::new(env, j_user_agent, "j_user_agent", "NXCS11u3")?;
    let log_dir = config_dir.as_path().join(RUST_LOG_DIR_NAME);
    let log_persistence = crate::logging::init(tracing_android::layer("ObscuraNative")?, Some(&log_dir));
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .map_err(|_| anyhow::format_err!("failed to install crypto provider"))?;
    let jvm = Arc::new(env.get_java_vm().context("failed to get JavaVM")?);
    let os_impl = Arc::new(AndroidOsImpl::new(jvm, class_cache.clone()));
    let manager = Manager::new(
        config_dir.as_path().into(),
        None, // TODO: https://linear.app/soveng/issue/OBS-2699/android-keychain-equivalent
        user_agent.as_str().into(),
        os_impl.clone(),
        None, // TODO: https://linear.app/soveng/issue/OBS-2699/android-keychain-equivalent
        log_persistence,
        true,
    )?;
    Ok(Global { manager, os_impl, runtime, class_cache })
}

/// cbindgen:ignore
/// Must be called on a Java thread
#[unsafe(no_mangle)]
pub extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_initialize(
    mut env: JNIEnv,
    _: JClass,
    j_config_dir: JString,
    j_user_agent: JString,
) -> jobject {
    tracing::info!(message_id = "PRXlxa85", "starting ffi initialization");
    let mut first_init = false;
    let global = GLOBAL.get_or_init(|| {
        first_init = true;
        let class_cache = ClassCache::new(&mut env).expect("creating class cache failed");
        let global = initialize(&mut env, &j_config_dir, &j_user_agent, class_cache).expect("`initialize` failed");
        tracing::info!(message_id = "Y6cNkZXW", "ffi initialized");
        global
    });
    if !first_init {
        tracing::error!(message_id = "sxsEyRKH", "ffi was already initialized")
    }
    env.new_object(global.class_cache.ffi_handle(), "()V", &[])
        .expect("failed to create FfiHandle")
        .into_raw()
}

fn json_ffi(global: &'static Global, env: &mut JNIEnv, j_json_cmd: &JString, j_future: &JObject) -> anyhow::Result<()> {
    let json_cmd = Utf8JavaStr::new(env, j_json_cmd, "j_json_cmd", "3ZxXYd09")?;
    let cmd = serde_json::from_str::<ManagerCmd>(json_cmd.as_str())?;
    // This extends the Java object's lifetime until dropped.
    let j_future = env.new_global_ref(&j_future)?;
    let _runtime_guard = global.runtime.enter();
    let jvm = env.get_java_vm()?;
    tokio::spawn(async move {
        let result = cmd.run(&global.manager).await;
        // This attaches the current thread to the JVM for the entire life of
        // the thread, which is significantly more performant than
        // attaching/detaching on each use. This will be a no-op if already
        // attached.
        //
        // Since it's attached as a "daemon thread", the life of this thread
        // won't extend the life of the JVM.
        match jvm.attach_current_thread_as_daemon() {
            Ok(mut env) => {
                if let Err(error) = signal_json_ffi_future(&global.class_cache, &mut env, j_future.as_obj(), result) {
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
pub extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_jsonFfi(
    mut env: JNIEnv,
    _: JClass,
    handle: JObject,
    j_json_cmd: JString,
    j_future: JObject,
) {
    let global = global_from_handle(&handle);
    if let Err(error) = json_ffi(global, &mut env, &j_json_cmd, &j_future) {
        tracing::error!(message_id = "jmx2DBFz", ?error, "`json_ffi` failed");
        throw_runtime_exception(&mut env, error);
    }
}

fn set_network_interface(env: &mut JNIEnv, global: &'static Global, j_name: &JString, j_index: jint) -> anyhow::Result<()> {
    let name = Utf8JavaStr::new(env, j_name, "j_name", "Quz8O0qu")?.to_string();
    let index = u32::try_from(j_index).and_then(PositiveU31::try_from).with_context(|| {
        tracing::error!(message_id = "qvDcd36g", "network interface index wasn't a positive u32");
        "network interface index wasn't a positive u32"
    })?;
    global.os_impl.set_network_interface(Some(NetworkInterface { name, index }));
    Ok(())
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_setNetworkInterface(
    mut env: JNIEnv,
    _: JClass,
    handle: JObject,
    j_name: JString,
    j_index: jint,
) {
    let global = global_from_handle(&handle);
    if let Err(error) = set_network_interface(&mut env, global, &j_name, j_index) {
        tracing::error!(message_id = "OOorBpQJ", ?error, "failed to set network interface: {error}");
    }
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_unsetNetworkInterface(_env: JNIEnv, _: JClass, handle: JObject) {
    let global = global_from_handle(&handle);
    global.os_impl.set_network_interface(None)
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
    let tag = Utf8JavaStr::new(env, j_tag, "j_tag", "Nfw9yJpe")?;
    let tag = tag.as_str();
    let message = Utf8JavaStr::new(env, j_message, "j_message", "gOpofoUs")?;
    let message = message.as_str();
    let message_id = Utf8JavaStr::new(env, j_message_id, "j_message_id", "vYRZ3DPv")?;
    let message_id = message_id.as_str();
    let throwable_string = Utf8JavaStr::from_nullable(env, j_throwable_string, "j_throwable_string", "5BNd8Tn1")?;
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

pub(super) async fn call_set_network_config(class_cache: Arc<ClassCache>, jvm: Arc<JavaVM>, json: String) -> Result<OwnedFd, ()> {
    tokio::task::spawn_blocking(move || {
        let mut env = jvm
            .attach_current_thread_as_daemon()
            .map_err(|error| tracing::error!(message_id = "c5B2cENp", ?error, "failed to attach thread to JVM: {error}"))?;
        let json_str = env
            .new_string(json)
            .map_err(|error| tracing::error!(message_id = "H6mOZNvn", ?error, "failed to create JNI string: {error}"))?;
        let j_fd = env
            .call_static_method(
                class_cache.vpn_service(),
                "ffiSetNetworkConfig",
                "(Ljava/lang/String;)I",
                &[JValue::Object(&json_str.into())],
            )
            .map_err(|error| tracing::error!(message_id = "oP7aSb3t", ?error, "failed to call ffiSetNetworkConfig: {error}"))?
            .i()
            .map_err(|error| tracing::error!(message_id = "orT0EU1k", ?error, "ffiSetNetworkConfig did not return an int: {error}"))?;
        if j_fd >= 0 {
            // SAFETY: `detachFd` surrendered ownership of the FD on the Kotlin side. No cleanup required besides `close`.
            Ok(unsafe { OwnedFd::from_raw_fd(j_fd) })
        } else {
            Err(())
        }
    })
    .await
    .expect("spawn_blocking panicked")
}

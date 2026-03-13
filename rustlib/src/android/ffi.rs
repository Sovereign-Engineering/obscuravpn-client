use super::{
    future::signal_json_ffi_future,
    os_impl::{AndroidOsImpl, SetNetworkConfigSender},
    util::{Utf8JavaStr, throw_runtime_exception},
};
use crate::{manager::Manager, manager_cmd::ManagerCmd, net::NetworkInterface, positive_u31::PositiveU31};
use anyhow::Context as _;
use jni::{
    JNIEnv, JavaVM,
    objects::{JClass, JObject, JString, JValue},
    sys::{jint, jlong},
};
use once_cell::sync::OnceCell;
use std::{
    ffi::c_void,
    os::fd::{FromRawFd as _, OwnedFd},
    sync::Arc,
};
use tokio::runtime::Runtime;

pub struct Global {
    pub manager: Arc<Manager>,
    pub os_impl: Arc<AndroidOsImpl>,
    pub runtime: Runtime,
}

static GLOBAL: OnceCell<Global> = OnceCell::new();

pub(super) fn global() -> anyhow::Result<&'static Global> {
    GLOBAL.get().context("ffi global not initialized")
}

fn jlong_from_ptr(ptr: *mut c_void) -> jlong {
    const _: () = assert!(jlong::BITS == usize::BITS, "jlong and pointer size mismatch");
    ptr as jlong
}

fn ptr_from_jlong(val: jlong) -> *mut c_void {
    const _: () = assert!(jlong::BITS == usize::BITS, "jlong and pointer size mismatch");
    val as *mut c_void
}

const RUST_LOG_DIR_NAME: &str = "rust-log";

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
    if let Err(error) = super::class_cache::init(&mut env) {
        throw_runtime_exception(&mut env, error);
    }
    jni::sys::JNI_VERSION_1_6
}

fn initialize(env: &mut JNIEnv, j_config_dir: &JString, j_user_agent: &JString) -> anyhow::Result<Global> {
    let runtime = Runtime::new().expect("Failed to create tokio runtime");
    let _runtime_guard = runtime.enter();
    let config_dir = Utf8JavaStr::new(env, j_config_dir, "j_config_dir")?;
    let user_agent = Utf8JavaStr::new(env, j_user_agent, "j_user_agent")?;
    let log_dir = config_dir.as_path().join(RUST_LOG_DIR_NAME);
    let log_persistence = crate::logging::init(tracing_android::layer("ObscuraNative")?, Some(&log_dir));
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .map_err(|_| anyhow::format_err!("failed to install crypto provider"))?;
    let jvm = env.get_java_vm().context("failed to get JavaVM")?;
    let os_impl = Arc::new(AndroidOsImpl::new(jvm));
    let manager = Manager::new(
        config_dir.as_path().into(),
        None, // TODO: https://linear.app/soveng/issue/OBS-2699/android-keychain-equivalent
        user_agent.as_str().into(),
        os_impl.clone(),
        None, // TODO: https://linear.app/soveng/issue/OBS-2699/android-keychain-equivalent
        log_persistence,
        true,
    )?;
    Ok(Global { manager, os_impl, runtime })
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
    if let Err(error) = GLOBAL.get_or_try_init(|| -> anyhow::Result<_> {
        let global = initialize(&mut env, &j_config_dir, &j_user_agent).context("`initialize` failed")?;
        first_init = true;
        Ok(global)
    }) {
        throw_runtime_exception(&mut env, error);
    }
    if !first_init {
        throw_runtime_exception(&mut env, "manager already initialized");
    }
}

fn json_ffi(env: &mut JNIEnv, j_json_cmd: &JString, j_future: &JObject) -> anyhow::Result<()> {
    let json_cmd = Utf8JavaStr::new(env, j_json_cmd, "j_json_cmd")?;
    let cmd = serde_json::from_str::<ManagerCmd>(json_cmd.as_str())?;
    // This extends the Java object's lifetime until dropped.
    let j_future = env.new_global_ref(&j_future)?;
    let manager = &global()?.manager;
    let _runtime_guard = global()?.runtime.enter();
    let jvm = env.get_java_vm()?;
    tokio::spawn(async move {
        let result = cmd.run(manager).await;
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

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_setNetworkInterface(
    mut env: JNIEnv,
    _: JClass,
    j_name: JString,
    j_index: jint,
) {
    let Ok(name) = Utf8JavaStr::new(&mut env, &j_name, "j_name").map(|s| s.to_string()).inspect_err(|error| {
        tracing::error!(
            message_id = "Quz8O0qu",
            ?error,
            "failed to get UTF8 string for network interface name: {error}"
        );
    }) else {
        return;
    };
    let Ok(index) = u32::try_from(j_index).and_then(PositiveU31::try_from) else {
        tracing::error!(message_id = "qvDcd36g", "network interface index wasn't a positive u32");
        return;
    };
    match global() {
        Ok(global) => global.os_impl.set_network_interface(Some(NetworkInterface { name, index })),
        Err(error) => tracing::error!(message_id = "zrtAAFEw", ?error, "failed to get Os impl to set network interface: {error}"),
    }
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_unsetNetworkInterface(_env: JNIEnv, _: JClass) {
    match global() {
        Ok(global) => global.os_impl.set_network_interface(None),
        Err(error) => tracing::error!(
            message_id = "gJEv6VGp",
            ?error,
            "failed to get Os impl to unset network interface: {error}"
        ),
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

pub(super) fn call_set_network_config(jvm: &JavaVM, json: &str, tx: SetNetworkConfigSender) -> Result<(), ()> {
    let mut env = jvm
        .attach_current_thread_as_daemon()
        .map_err(|error| tracing::error!(message_id = "c5B2cENp", ?error, "failed to attach thread to JVM: {error}"))?;
    let class = super::class_cache::get().map_err(|error| tracing::error!(message_id = "JvCqA8Jf", ?error, "failed to get class cache: {error}"))?;
    let json_str = env
        .new_string(json)
        .map_err(|error| tracing::error!(message_id = "H6mOZNvn", ?error, "failed to create JNI string: {error}"))?;
    let context = Box::into_raw(Box::new(tx)).cast::<c_void>();
    env.call_static_method(
        class.vpn_service(),
        "ffiSetNetworkConfig",
        "(Ljava/lang/String;J)V",
        &[JValue::Object(&json_str.into()), JValue::Long(jlong_from_ptr(context))],
    )
    .map_err(|error| {
        tracing::error!(message_id = "oP7aSb3t", ?error, "failed to call ffiSetNetworkConfig: {error}");
        // SAFETY: `context` was created via `Box<SetNetworkConfigSender>::into_raw` and ownership was not transferred to Java.
        unsafe { drop(Box::from_raw(context.cast::<SetNetworkConfigSender>())) };
    })?;
    Ok(())
}

/// Must be called exactly once per call to call_set_network_config with the same `context` as `j_context`.
/// `j_fd >= 0` means success and transfers fd ownership to Rust. `j_fd < 0` means failure.
///
/// cbindgen:ignore
#[unsafe(no_mangle)]
pub extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_setNetworkConfigDone(_env: JNIEnv, _: JClass, j_context: jlong, j_fd: jint) {
    // SAFETY: `j_context` was created via `Box<SetNetworkConfigSender>::into_raw` in `call_set_network_config`
    let sender = unsafe {
        let context = ptr_from_jlong(j_context);
        Box::from_raw(context.cast::<SetNetworkConfigSender>())
    };

    let result = (j_fd >= 0)
        .then(|| {
            // SAFETY: `detachFd` surrendered ownership of the FD on the Kotlin side. No cleanup required besides `close`.
            unsafe { OwnedFd::from_raw_fd(j_fd) }
        })
        .ok_or(());
    let _ = sender.send(result);
}

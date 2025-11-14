use crate::ffi_helpers::FfiBytes;
use crate::manager::Manager;
use crate::manager_cmd::ManagerCmd;
use crate::manager_cmd::ManagerCmdErrorCode;
use jni::JNIEnv;
use jni::objects::{JClass, JObject, JString, JValue};
use std::ffi::c_void;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, OnceLock};
use tokio::runtime::Runtime;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Registry};

static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| Runtime::new().unwrap());
static GLOBAL: OnceLock<Arc<Manager>> = OnceLock::new();

/// cbindgen:ignore
extern "C" fn receive_cb(_ffi_bytes: FfiBytes) -> () {
    // TODO
    tracing::info!("receive_cb")
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub extern "C" fn JNI_OnLoad(_vm: *mut jni::sys::JavaVM, _reserved: *mut c_void) -> jni::sys::jint {
    let android_layer = tracing_android::layer("ObscuraNative").expect("failed to create tracing-android layer");

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    Registry::default().with(filter).with(android_layer).init();

    std::panic::set_hook(Box::new(|info| {
        let loc = info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
            .unwrap_or_else(|| "<unknown>".into());
        let msg = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(|s| s.as_str()))
            .unwrap_or("panic payload not str");
        let bt = std::backtrace::Backtrace::force_capture();
        tracing::error!("panic at {loc}: {msg}\n{bt}");
    }));

    jni::sys::JNI_VERSION_1_6
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_initialize(
    mut env: JNIEnv,
    _: JClass,
    java_config_dir: JString,
    java_user_agent: JString,
) -> () {
    let mut first_init = false;
    GLOBAL.get_or_init(|| {
        rustls::crypto::aws_lc_rs::default_provider()
            .install_default()
            .expect("Failed to install aws-lc crypto provider");

        let config_dir: String = env.get_string(&java_config_dir).expect("first argument must be a string").into();
        let user_agent = env.get_string(&java_user_agent).expect("second argument must be a string");

        match Manager::new(
            PathBuf::from(config_dir),
            None, // TODO
            user_agent.into(),
            &RUNTIME,
            receive_cb,
            None, // TODO
            None, // TODO
        ) {
            Ok(c) => {
                first_init = true;
                c
            }
            Err(err) => panic!("ffi initialization failed: could not load config: {err}"),
        }
    });

    if !first_init {
        env.throw_new("java/lang/RuntimeException", "already initialized")
            .expect("must throw an already initialized exception");
    }
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_jsonFfi(
    mut env: JNIEnv,
    _: JClass,
    java_json: JString,
    java_future: JObject,
) {
    let json_cmd: String = env.get_string(&java_json).expect("first argument must be a string").into();
    let future_global = env.new_global_ref(&java_future).expect("global ref to second argument must be available");
    let jvm = env.get_java_vm().expect("jvm must be available");

    let cmd: ManagerCmd = match serde_json::from_str(&json_cmd) {
        Ok(cmd) => cmd,
        Err(error) => {
            env.throw_new("java/lang/RuntimeException", error.to_string())
                .expect("must throw a java exception on manager error");
            return;
        }
    };

    RUNTIME.spawn(async move {
        let manager = GLOBAL.get().expect("ffi manager not initialized").clone();

        let result = cmd.run(&manager).await;

        // important to call after await to ensure proper thread
        let mut env = jvm.attach_current_thread_as_daemon().expect("failed to attach jvm to current thread");
        let future = future_global.as_obj();

        let json_result = result.and_then(|ok| {
            serde_json::to_string(&ok).map_err(|err| {
                tracing::error!("failed to serialize manager result to json {err}");
                return ManagerCmdErrorCode::Other;
            })
        });

        match json_result {
            Ok(ok) => {
                env.call_method(
                    future,
                    "complete",
                    "(Ljava/lang/Object;)Z",
                    &[JValue::Object(&JObject::from(env.new_string(ok).expect("must become java string")))],
                )
                .expect("must have called complete");
            }
            Err(err) => {
                let exception = env
                    .new_object(
                        "net/obscura/vpnclientapp/client/JsonFfiException",
                        "(Ljava/lang/String;)V",
                        &[JValue::Object(&JObject::from(
                            env.new_string(serde_json::to_string(&err).expect("must be convertible to JSON"))
                                .expect("must become java string"),
                        ))],
                    )
                    .expect("must create exception");

                // TODO figure out error codes and things
                env.call_method(
                    future,
                    "completeExceptionally",
                    "(Ljava/lang/Throwable;)V",
                    &[JValue::Object(&JObject::from(exception))],
                )
                .expect("must have called completeExceptionally");
            }
        }
    });
}

use crate::ffi_helpers::FfiBytes;
use crate::manager::Manager;
use crate::manager_cmd::ManagerCmd;
use crate::manager_cmd::ManagerCmdErrorCode;
use crate::net::NetworkInterface;
use crate::positive_u31::PositiveU31;
use crate::tokio::AbortOnDrop;
use jni::JNIEnv;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::jint;
use nix::errno::Errno;
use nix::net::if_::if_indextoname;
use nix::unistd;
use std::ffi::c_void;
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd};
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use tokio::io::unix::AsyncFd;
use tokio::runtime::Runtime;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Registry};

static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| Runtime::new().unwrap());
static GLOBAL: OnceLock<Arc<Manager>> = OnceLock::new();
static TUNNEL: Mutex<Option<Tunnel>> = Mutex::new(None);

struct Tunnel {
    fd: OwnedFd,
    _read_loop_task: AbortOnDrop,
}

impl Tunnel {
    fn spawn(fd: OwnedFd) -> Self {
        let fd_watcher = AsyncFd::new(fd.as_raw_fd()).expect("failed to watch tun");

        let read_loop_task = RUNTIME.spawn(async move {
            let manager = GLOBAL.get().expect("ffi manager not initialized").clone();

            // technically can't be bigger than MTU but just in case
            let mut buf = Box::new([0; 4096]);

            loop {
                match fd_watcher.readable().await {
                    Ok(mut guard) => match unistd::read(&fd_watcher, &mut buf[..]) {
                        Ok(n) => {
                            if n > 0 {
                                manager.send_packet(&mut buf[..n]);
                            }
                        }
                        Err(Errno::EAGAIN) => {
                            guard.clear_ready();
                        }
                        Err(error) => {
                            tracing::error!(message_id = "eagh6Noh", ?error, "failed to read from tun");
                            break;
                        }
                    },
                    Err(error) => {
                        tracing::error!(message_id = "r5N6izcO", ?error, "failed to wait for tun to become readable");
                        break;
                    }
                }
            }
        });

        Self { fd, _read_loop_task: read_loop_task.into() }
    }
}

/// cbindgen:ignore
extern "C" fn receive_cb(ffi_bytes: FfiBytes) -> () {
    if let Some(tun) = &*TUNNEL.lock().unwrap() {
        if let Err(error) = unistd::write(&tun.fd, &ffi_bytes.as_slice()) {
            tracing::error!(message_id = "W0sOhigq", ?error, "writing packet to tun failed");
        }
    }
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub extern "C" fn JNI_OnLoad(_vm: *mut jni::sys::JavaVM, _reserved: *mut c_void) -> jni::sys::jint {
    let android_layer = tracing_android::layer("ObscuraNative").expect("failed to create `tracing-android` layer");

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
        tracing::error!(message_id = "W6fhvnSf", "panic at {loc}: {msg}\n{bt}");
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

        let config_dir: String = env.get_string(&java_config_dir).expect("`java_config_dir` wasn't a string").into();
        let user_agent = env.get_string(&java_user_agent).expect("`java_user_agent` wasn't a string");

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
            .expect("failed to throw exception");
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
    let json_cmd: String = env.get_string(&java_json).expect("`java_json` wasn't a string").into();
    let future_global = env.new_global_ref(&java_future).expect("global ref to `java_future` wasn't available");
    let jvm = env.get_java_vm().expect("jvm wasn't available");

    let cmd: ManagerCmd = match serde_json::from_str(&json_cmd) {
        Ok(cmd) => cmd,
        Err(error) => {
            env.throw_new("java/lang/RuntimeException", error.to_string())
                .expect("failed to throw exception");
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
            serde_json::to_string(&ok).map_err(|error| {
                tracing::error!(message_id = "hP0R8zXa", ?error, "could not serialize successful json cmd result");
                return ManagerCmdErrorCode::Other;
            })
        });

        match json_result {
            Ok(ok) => {
                env.call_method(
                    future,
                    "complete",
                    "(Ljava/lang/Object;)Z",
                    &[JValue::Object(&JObject::from(
                        env.new_string(ok).expect("failed to convert serailized json to java string"),
                    ))],
                )
                .expect("failed to call `complete`");
            }
            Err(err) => {
                let exception = env
                    .new_object(
                        "net/obscura/vpnclientapp/client/JsonFfiException",
                        "(Ljava/lang/String;)V",
                        &[JValue::Object(&JObject::from(
                            env.new_string(serde_json::to_string(&err).expect("could not serialize failed json cmd result"))
                                .expect("failed to convert serialized json to java string"),
                        ))],
                    )
                    .expect("failed to construct exception");

                // TODO figure out error codes and things
                env.call_method(
                    future,
                    "completeExceptionally",
                    "(Ljava/lang/Throwable;)V",
                    &[JValue::Object(&JObject::from(exception))],
                )
                .expect("failed to call `completeExceptionally`");
            }
        }
    });
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_setNetworkInterfaceIndex(
    _env: JNIEnv,
    _: JClass,
    java_index: jint,
) -> () {
    if java_index <= 0 {
        GLOBAL.get().expect("global manager not initialized").set_network_interface(None);
    } else {
        let index = u32::try_from(java_index)
            .and_then(PositiveU31::try_from)
            .expect("network interface index wasn't a positive u32");
        let name = if_indextoname(index.into())
            .expect("failed to get network interface name for index")
            .into_string()
            .expect("failed to convert network interface name to string");
        let ni = NetworkInterface { name, index };

        GLOBAL.get().expect("global manager not initialized").set_network_interface(Some(ni));
    }
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_startTunnel(_env: JNIEnv, _: JClass, java_fd: jint) -> () {
    // SAFETY:
    // - `detachFd` surrenders ownership of the FD on the Kotlin side
    // - No cleanup required besides `close`
    let fd = unsafe { OwnedFd::from_raw_fd(java_fd) };
    TUNNEL.lock().unwrap().replace(Tunnel::spawn(fd));
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_stopTunnel(_env: JNIEnv, _: JClass) -> () {
    TUNNEL.lock().unwrap().take();
}

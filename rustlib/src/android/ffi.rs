use crate::ffi_helpers::FfiBytes;
use crate::manager::Manager;
use crate::manager_cmd::ManagerCmd;
use crate::manager_cmd::ManagerCmdErrorCode;
use crate::net::NetworkInterface;
use jni::JNIEnv;
use jni::objects::{JClass, JObject, JString, JValue};
use jni::sys::jint;
use nix::errno::Errno;
use nix::net::if_::if_indextoname;
use nix::unistd;
use std::ffi::c_void;
use std::num::NonZeroU32;
use std::os::fd::BorrowedFd;
use std::os::fd::RawFd;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex, OnceLock};
use tokio::io::unix::AsyncFd;
use tokio::runtime::Runtime;
use tokio::task::JoinHandle;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use tracing_subscriber::{EnvFilter, Registry};

struct TunnelRxTx {
    pub fd: RawFd,
    pub handle_tx: JoinHandle<()>,
}

static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| Runtime::new().unwrap());
static GLOBAL: OnceLock<Arc<Manager>> = OnceLock::new();
static TUNNEL_RX_TX: OnceLock<Mutex<Option<TunnelRxTx>>> = OnceLock::new();

impl TunnelRxTx {
    pub fn spawn(fd: RawFd) -> Self {
        let bfd = unsafe { BorrowedFd::borrow_raw(fd) };
        let afd = Arc::new(AsyncFd::new(bfd).expect("failed to register fd"));

        let rafd = afd.clone();
        let handle_tx = RUNTIME.spawn(async move {
            let manager = GLOBAL.get().expect("ffi manager not initialized").clone();

            // technically can't be bigger than MTU but just in case
            let mut buf: Box<[u8; 4096]> = Box::new([0; 4096]);

            loop {
                match rafd.readable().await {
                    Ok(mut guard) => match unistd::read(bfd, &mut buf[..]) {
                        Ok(n) => {
                            if n > 0 {
                                manager.send_packet(&mut buf[..n]);
                            }
                        }

                        Err(Errno::EAGAIN) => {
                            guard.clear_ready();
                        }

                        Err(e) => {
                            tracing::error!("read from tunnel failed {e}");
                            break;
                        }
                    },

                    Err(e) => {
                        tracing::error!("readable wait failed {e}");
                        break;
                    }
                }
            }
        });

        Self { handle_tx, fd }
    }

    pub fn stop(self) {
        self.handle_tx.abort();

        if let Err(e) = unistd::close(self.fd) {
            tracing::error!("closing fd failed {e}");
        }
    }
}

/// cbindgen:ignore
extern "C" fn receive_cb(ffi_bytes: FfiBytes) -> () {
    let guard = TUNNEL_RX_TX.get().expect("tunnel global not initialized").lock().unwrap();

    if let Some(ref tun) = *guard {
        let bfd = unsafe { BorrowedFd::borrow_raw(tun.fd) };

        if let Err(e) = unistd::write(bfd, &ffi_bytes.as_slice()) {
            tracing::error!("writing packet failed {e}");
        }
    }
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub extern "C" fn JNI_OnLoad(_vm: *mut jni::sys::JavaVM, _reserved: *mut c_void) -> jni::sys::jint {
    TUNNEL_RX_TX.set(Mutex::new(None)).ok();

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
            .ok()
            .and_then(NonZeroU32::new)
            .expect("network interface index must be non-zero u32");
        let name = if_indextoname(index.into())
            .expect("network interface index must convert to name")
            .into_string()
            .expect("network interface name must convert to a string");
        let ni = NetworkInterface { name, index };

        GLOBAL.get().expect("global manager not initialized").set_network_interface(Some(ni));
    }
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_startTunnel(_env: JNIEnv, _: JClass, java_fd: jint) -> () {
    let tun = {
        let mut guard = TUNNEL_RX_TX.get().expect("tunnel global not initialized").lock().unwrap();
        let tun = guard.take();

        *guard = Some(TunnelRxTx::spawn(java_fd));

        tun
    };

    if let Some(tun) = tun {
        tun.stop();
    }
}

/// cbindgen:ignore
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_net_obscura_vpnclientapp_client_ObscuraLibrary_stopTunnel(_env: JNIEnv, _: JClass) -> () {
    let tun = {
        let mut guard = TUNNEL_RX_TX.get().expect("tunnel global not initialized").lock().unwrap();
        let tun = guard.take();

        *guard = None;

        tun
    };

    if let Some(tun) = tun {
        tun.stop();
    }
}

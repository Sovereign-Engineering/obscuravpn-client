use tracing_appender::non_blocking::{NonBlocking, WorkerGuard};
use tracing_subscriber::{
    Layer, Registry,
    filter::{EnvFilter, LevelFilter},
    layer::SubscriberExt as _,
    registry,
};

#[cfg(any(target_os = "android", target_os = "ios"))]
fn build_log_roller(log_dir: &str) -> anyhow::Result<(NonBlocking, WorkerGuard)> {
    use logroller::{Compression, LogRollerBuilder, Rotation, RotationSize, TimeZone};

    static LOG_FILE_NAME: &str = "rust-log.ndjson";
    const MAX_LOG_FILES: u64 = 24;
    const MAX_LOG_SIZE: u64 = 10_000_000;

    if log_dir.is_empty() {
        anyhow::bail!("no log dir specified");
    }
    LogRollerBuilder::new(log_dir, LOG_FILE_NAME)
        // The rotation often runs behind a bit, but at low log pressure
        // (i.e. not TRACE) it's good enough
        .rotation(Rotation::SizeBased(RotationSize::Bytes(MAX_LOG_SIZE)))
        .max_keep_files(MAX_LOG_FILES)
        .time_zone(TimeZone::UTC)
        // https://linux.die.net/man/1/xz
        .compression(Compression::XZ(2))
        .build()
        .map(NonBlocking::new)
        .map_err(Into::into)
}

#[cfg(not(any(target_os = "android", target_os = "ios")))]
fn build_log_roller(log_dir: &str) -> anyhow::Result<(NonBlocking, WorkerGuard)> {
    anyhow::bail!("specified log dir on a platform that doesn't support log persistence: {log_dir}")
}

// `EnvFilter` doesn't impl `Clone`
fn filter() -> EnvFilter {
    EnvFilter::from_default_env().add_directive(LevelFilter::INFO.into())
}

pub fn init(base_layer: impl Layer<Registry> + Send + Sync, persistence_dir: Option<&str>) -> *mut WorkerGuard {
    let registry = registry().with(base_layer.with_filter(filter()));
    let guard_ptr = if let Some((writer, guard)) = persistence_dir.and_then(|log_dir| {
        build_log_roller(log_dir)
            .inspect_err(|error| {
                tracing::error!(?error, "failed to initialize log persistence");
            })
            .ok()
    }) {
        let fs_layer = tracing_subscriber::fmt::Layer::default().json().with_writer(writer).with_filter(filter());
        tracing::subscriber::set_global_default(registry.with(fs_layer)).expect("failed to set global subscriber");
        Box::into_raw(Box::new(guard))
    } else {
        tracing::subscriber::set_global_default(registry).expect("failed to set global subscriber");
        std::ptr::null_mut()
    };
    tracing::info!("logging initialized");
    std::panic::set_hook(Box::new(|panic_info| {
        tracing::error!(message_id = "W6fhvnSf", "{panic_info}\n{:#}", std::backtrace::Backtrace::force_capture());
    }));
    tracing::info!("panic logging hook set");
    guard_ptr
}

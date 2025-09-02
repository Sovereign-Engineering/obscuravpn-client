use logroller::{Compression, LogRoller, LogRollerBuilder, Rotation, RotationSize, TimeZone};

static LOG_DIR_NAME: &str = "logs";
static LOG_FILE_NAME: &str = "rust-log.ndjson";
const MAX_LOG_FILES: u64 = 24;
const MAX_LOG_SIZE: u64 = 10_000_000;

fn container_dir() -> anyhow::Result<camino::Utf8PathBuf> {
    use anyhow::Context as _;
    use objc2_foundation::{NSFileManager, NSString};
    let manager = unsafe { NSFileManager::defaultManager() };
    let group_identifier = NSString::from_str("group.net.obscura.vpn-client-app-ios");
    let url = unsafe { manager.containerURLForSecurityApplicationGroupIdentifier(&group_identifier) }.context("no container url for group")?;
    let path = url.to_file_path().context("url didn't contain valid path")?;
    Ok(path.try_into()?)
}

pub fn log_dir() -> anyhow::Result<camino::Utf8PathBuf> {
    Ok(container_dir()?.join(LOG_DIR_NAME))
}

pub(super) fn build_log_roller() -> anyhow::Result<LogRoller> {
    log_dir().and_then(|log_dir| {
        LogRollerBuilder::new(log_dir.as_str(), LOG_FILE_NAME)
            // The rotation often runs behind a bit, but at low log pressure
            // (i.e. not TRACE) it's good enough
            .rotation(Rotation::SizeBased(RotationSize::Bytes(MAX_LOG_SIZE)))
            .max_keep_files(MAX_LOG_FILES)
            .time_zone(TimeZone::UTC)
            // https://linux.die.net/man/1/xz
            .compression(Compression::XZ(2))
            .build()
            .map_err(Into::into)
    })
}

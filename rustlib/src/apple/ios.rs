use crate::ffi_helpers::*;
use anyhow::Context;
use logroller::{Compression, LogRoller, LogRollerBuilder, Rotation, RotationSize, TimeZone};
use std::sync::OnceLock;

static LOG_FILE_NAME: &str = "rust-log.ndjson";
const MAX_LOG_FILES: u64 = 24;
const MAX_LOG_SIZE: u64 = 10_000_000;

pub static IOS_LOG_DIR: OnceLock<String> = OnceLock::new();

pub(super) fn build_log_roller(log_dir: &FfiStr) -> anyhow::Result<LogRoller> {
    let Some(log_dir) = Some(log_dir.as_str()).filter(|s| !s.is_empty()) else {
        anyhow::bail!("no log dir specified")
    };
    IOS_LOG_DIR.set(log_dir.to_string()).ok().context("failed to set iOS global static")?;
    LogRollerBuilder::new(log_dir, LOG_FILE_NAME)
        // The rotation often runs behind a bit, but at low log pressure
        // (i.e. not TRACE) it's good enough
        .rotation(Rotation::SizeBased(RotationSize::Bytes(MAX_LOG_SIZE)))
        .max_keep_files(MAX_LOG_FILES)
        .time_zone(TimeZone::UTC)
        // https://linux.die.net/man/1/xz
        .compression(Compression::XZ(2))
        .build()
        .map_err(Into::into)
}

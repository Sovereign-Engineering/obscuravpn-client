mod builder;
pub mod bundle_info;
#[cfg(target_os = "linux")]
pub mod client;
pub mod debug_info;
pub mod dns;
pub mod http;
pub mod service;
pub mod task;
#[cfg(target_os = "windows")]
mod windows_acl;
pub mod zipper;

use self::{builder::DebugBundleBuilder, bundle_info::BundleInfo, debug_info::DebugInfo};
use camino::Utf8PathBuf;
use chrono::{SecondsFormat, Utc};
use rand::Rng;
use rand::distributions::Alphanumeric;

pub const DIR_PREFIX: &str = "obscura-debug-bundle-";

fn temp_bundle_dir_path() -> Result<Utf8PathBuf, ()> {
    let temp_dir =
        Utf8PathBuf::from_path_buf(std::env::temp_dir()).map_err(|path| tracing::error!(message_id = "hV4tNq2X", ?path, "temp dir is not utf-8"))?;
    let dir_name: String = rand::thread_rng().sample_iter(&Alphanumeric).take(24).map(char::from).collect();
    Ok(temp_dir.join(format!("{DIR_PREFIX}{dir_name}")))
}

/// 0700, so nobody else can observe or modify the dir until the caller widens the modes.
#[cfg(unix)]
pub(crate) async fn make_private_temp_dir() -> Result<Utf8PathBuf, ()> {
    let path = temp_bundle_dir_path()?;
    let mut builder = tokio::fs::DirBuilder::new();
    builder.mode(0o700);
    builder
        .create(&path)
        .await
        .map_err(|error| tracing::error!(message_id = "gT5cWn9M", ?error, %path, "failed to create private temp dir"))?;
    Ok(path)
}

#[cfg(target_os = "windows")]
pub(crate) async fn make_users_readable_temp_dir() -> Result<Utf8PathBuf, ()> {
    let path = temp_bundle_dir_path()?;
    windows_acl::create_users_readable_dir(&path)
        .map_err(|error| tracing::error!(message_id = "sQ7fNd3B", ?error, %path, "failed to create users readable temp dir"))?;
    Ok(path)
}

pub(crate) async fn try_copy_dir_contents_recursive(src: &Utf8Path, dst: &Utf8Path) {
    if let Err(error) = tokio::fs::create_dir_all(dst).await {
        tracing::error!(message_id = "nY7dFq3S", ?error, %dst, "failed to create dir for copy");
        return;
    }
    let mut queue = vec![(src.to_owned(), dst.to_owned())];
    while let Some((src, dst)) = queue.pop() {
        let Ok(mut entries) = tokio::fs::read_dir(&src)
            .await
            .map_err(|error| tracing::error!(message_id = "pZ3xGt7J", ?error, %src, "failed to read dir for copy"))
        else {
            continue;
        };
        loop {
            let entry = match entries.next_entry().await {
                Ok(Some(entry)) => entry,
                Ok(None) => break,
                Err(error) => {
                    tracing::error!(message_id = "kT8mBv3Q", ?error, %src, "failed to read dir entry; skipping rest of dir");
                    break;
                }
            };
            let file_name = entry.file_name();
            let Some(name) = file_name.to_str() else {
                tracing::warn!(message_id = "mC5tYb9W", ?file_name, %src, "skipping file with non-utf-8 name");
                continue;
            };
            let src_path = src.join(name);
            let dst_path = dst.join(name);
            let Ok(file_type) = entry
                .file_type()
                .await
                .map_err(|error| tracing::error!(message_id = "uW6kDp2M", ?error, %src_path, "failed to get file type; skipping"))
            else {
                continue;
            };
            if file_type.is_dir() {
                if let Err(error) = tokio::fs::create_dir(&dst_path).await {
                    tracing::error!(message_id = "eK9mVw2C", ?error, dst =% dst_path, "failed to create dir for copy");
                    continue;
                }
                queue.push((src_path, dst_path));
            } else if file_type.is_file() {
                let _ = tokio::fs::copy(&src_path, &dst_path)
                    .await
                    .map_err(|error| tracing::error!(message_id = "jR4nTv8Y", ?error, %src_path, "failed to copy file; skipping"));
            } else {
                tracing::warn!(message_id = "wJ2uJZfF", ?file_type, %src_path, "skipping non-regular file");
            }
        }
    }
}

// TODO: https://linear.app/soveng/issue/OBS-3095/cross-platform-debug-archive-story
// TODO: Deprecated, switch to create_service_debug_bundle: https://linear.app/soveng/issue/OBS-3918/debug-bundle-v3
pub fn create_debug_bundle(
    user_feedback: Option<String>,
    bundle_info: BundleInfo,
    debug_info: DebugInfo,
    rust_log_dir: Option<Utf8PathBuf>,
    android_cache_dir: Option<Utf8PathBuf>,
) -> anyhow::Result<Utf8PathBuf> {
    let bundle_dir = if cfg!(target_os = "android") {
        // `std::env::temp_dir` only returns the app cache dir on Android 13+ (SDK 33), but we support Android 12 (SDK 31/32).
        android_cache_dir
    } else {
        // Explicitly ignore this field outside of Android to prevent potential privilege escalation vulnerabilities.
        None
    };
    let bundle_timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let mut archive = DebugBundleBuilder::new(bundle_dir, &bundle_timestamp)?;
    archive.add_json(
        "info",
        &BundleInfo { bundle_timestamp: Some(bundle_timestamp.to_string()), ..bundle_info },
    );
    archive.add_json("ne-debug-info", &debug_info);
    if let Some(user_feedback) = user_feedback {
        archive.add_txt("user-feedback", &user_feedback);
    }
    if let Some(rust_log_dir) = rust_log_dir {
        archive.add_path("rust-log", None, &rust_log_dir);
    }
    if cfg!(target_os = "android") {
        // This isn't guaranteed to work, but Android unfortunately doesn't
        // provide a proper API for this.
        archive.add_cmd("logcat", "txt", diva::Command::parse("logcat -d"));
    }
    #[cfg(target_os = "android")]
    if let Some(json) = crate::android::process_exit_reasons_json() {
        archive.add_bytes("process-exit-reasons", "json", json.as_bytes());
    }
    archive.finish()
}

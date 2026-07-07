mod builder;
pub mod bundle_info;
pub mod debug_info;
pub mod dns;
pub mod http;
pub mod task;
mod zipper;

use self::{builder::DebugBundleBuilder, bundle_info::BundleInfo, debug_info::DebugInfo};
use camino::{Utf8Path, Utf8PathBuf};
use chrono::{SecondsFormat, Utc};

// TODO: https://linear.app/soveng/issue/OBS-3095/cross-platform-debug-archive-story
pub fn create_debug_bundle(
    user_feedback: Option<&str>,
    bundle_info: BundleInfo,
    debug_info: DebugInfo,
    rust_log_dir: Option<&Utf8Path>,
) -> anyhow::Result<Utf8PathBuf> {
    let bundle_timestamp = Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true);
    let mut archive = DebugBundleBuilder::new(&bundle_timestamp)?;
    archive.add_json(
        "info",
        &BundleInfo { bundle_timestamp: Some(bundle_timestamp.to_string()), ..bundle_info },
    );
    archive.add_json("ne-debug-info", &debug_info);
    if let Some(user_feedback) = user_feedback {
        archive.add_txt("user-feedback", user_feedback);
    }
    if let Some(rust_log_dir) = rust_log_dir {
        archive.add_path("rust-log", None, rust_log_dir);
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

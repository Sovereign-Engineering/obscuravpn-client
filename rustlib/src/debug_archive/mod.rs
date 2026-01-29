mod builder;
mod zipper;

use self::builder::DebugArchiveBuilder;
use crate::config::ConfigDebug;
use camino::{Utf8Path, Utf8PathBuf};

// TODO: https://linear.app/soveng/issue/OBS-3095/cross-platform-debug-archive-story
pub fn create_debug_archive(user_feedback: Option<&str>, config: &ConfigDebug, rust_log_dir: Option<&Utf8Path>) -> anyhow::Result<Utf8PathBuf> {
    let mut archive = DebugArchiveBuilder::new()?;
    archive.add_json("config", config);
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
    archive.finish()
}

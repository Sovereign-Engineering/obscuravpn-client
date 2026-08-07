use super::try_copy_dir_contents_recursive;
use super::zipper::Zipper;
use camino::{Utf8Path, Utf8PathBuf};

pub async fn populate_client_debug_bundle(dir: &Utf8Path, user_feedback: Option<&str>, client_log_dir: Option<&Utf8Path>) {
    if let Some(user_feedback) = user_feedback {
        let _ = tokio::fs::write(dir.join("user-feedback.txt"), user_feedback)
            .await
            .map_err(|error| tracing::error!(message_id = "cJ3nZk8V", ?error, "failed to write user feedback into debug bundle"));
    }
    if let Some(client_log_dir) = client_log_dir {
        try_copy_dir_contents_recursive(client_log_dir, &dir.join("logs-client")).await;
    }
    tracing::info!(message_id = "rW6kTm3B", %dir, "populated client debug bundle contents");
}

pub(crate) async fn zip_and_remove_dir(src: &Utf8Path, dst_parent: &Utf8Path, name: String) -> Result<Utf8PathBuf, ()> {
    let src_owned = src.to_owned();
    let dst_parent = dst_parent.to_owned();
    let result = match tokio::task::spawn_blocking(move || Zipper::zip_dir(&src_owned, &dst_parent, name)).await {
        Ok(Ok(zip_path)) => {
            tracing::info!(message_id = "xH5cPj9K", path =% zip_path, "created debug bundle zip");
            Ok(zip_path)
        }
        Ok(Err(error)) => {
            tracing::error!(message_id = "aQ2wFj6R", ?error, %src, "failed to zip dir");
            Err(())
        }
        Err(error) => {
            tracing::error!(message_id = "dV7gMs2H", ?error, "zip task panicked");
            Err(())
        }
    };
    if let Err(error) = tokio::fs::remove_dir_all(src).await {
        tracing::error!(message_id = "pV4nWq9Z", ?error, %src, "failed to remove dir after zipping");
    }
    result
}

#[cfg(unix)]
use super::make_private_temp_dir;
#[cfg(target_os = "windows")]
use super::make_users_readable_temp_dir;
use super::try_copy_dir_contents_recursive;
use crate::config::ConfigDebug;
use crate::constants::DEFAULT_API_DOMAIN;
use crate::debug_bundle::debug_info::DebugInfo;
use crate::debug_bundle::populate_tasks::populate_debug_tasks;
use crate::debug_bundle::{DebugBundleSide, try_write_json_file};
use crate::manager_cmd::PeerUid;
use crate::net::NetworkInterface;
use camino::{Utf8Path, Utf8PathBuf};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct ServiceDebugBundleToken(Uuid);

#[derive(Debug, Serialize)]
pub struct NetworkInfo {
    pub network_interface: Option<NetworkInterface>,
    pub network_interface_mtu: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDebugBundleHandle {
    pub path: Utf8PathBuf,
    pub token: ServiceDebugBundleToken,
}

impl ServiceDebugBundleHandle {
    // On unix the dir starts private and make_user_readable widens it once the bundle is complete.
    // On Windows entries inherit the dir's DACL as they are created, so the dir gets its final one up front.
    async fn mkdir() -> Result<Self, ()> {
        #[cfg(unix)]
        let path = make_private_temp_dir().await?;
        #[cfg(target_os = "windows")]
        let path = make_users_readable_temp_dir().await?;
        Ok(Self { path, token: ServiceDebugBundleToken(Uuid::new_v4()) })
    }

    // Files become 0400 owned by the requester (if known); directories stay root-owned at 0755 so the requester can read and traverse but not write them, which keeps the service's later recursive delete safe from a planted symlink. The top dir is done last, so nobody else can see an unfinished bundle.
    #[cfg(unix)]
    async fn make_user_readable(&self, peer_uid: PeerUid) -> Result<(), ()> {
        use std::os::unix::fs::PermissionsExt as _;

        let mut queue = vec![self.path.clone()];
        while let Some(dir) = queue.pop() {
            let Ok(mut entries) = tokio::fs::read_dir(&dir)
                .await
                .map_err(|error| tracing::error!(message_id = "pT6kJd9W", ?error, %dir, "failed to read dir while adjusting permissions"))
            else {
                continue;
            };
            loop {
                let entry = match entries.next_entry().await {
                    Ok(Some(entry)) => entry,
                    Ok(None) => break,
                    Err(error) => {
                        tracing::error!(message_id = "qF3mZv7B", ?error, %dir, "failed to read dir entry while adjusting permissions");
                        break;
                    }
                };
                let file_name = entry.file_name();
                let Some(name) = file_name.to_str() else {
                    tracing::warn!(message_id = "yG6vBn3X", ?file_name, %dir, "skipping file with non-utf-8 name while adjusting permissions");
                    continue;
                };
                let path = dir.join(name);
                let Ok(file_type) = entry
                    .file_type()
                    .await
                    .map_err(|error| tracing::error!(message_id = "cD5nRw8M", ?error, %path, "failed to get file type while adjusting permissions"))
                else {
                    continue;
                };
                if file_type.is_dir() {
                    queue.push(path.clone());
                    let _ = tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                        .await
                        .map_err(|error| tracing::error!(message_id = "hK3nDv7M", ?error, %path, "failed to adjust dir permissions"));
                } else if file_type.is_file() {
                    let _ = tokio::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o400))
                        .await
                        .map_err(|error| tracing::error!(message_id = "vK2xNs5D", ?error, %path, "failed to adjust file permissions"));
                    let _ = std::os::unix::fs::lchown(&path, Some(peer_uid.0), None)
                        .map_err(|error| tracing::error!(message_id = "nQ5wPj2K", ?error, %path, "failed to chown file to requester"));
                } else {
                    continue;
                }
            }
        }
        tokio::fs::set_permissions(&self.path, std::fs::Permissions::from_mode(0o755))
            .await
            .map_err(|error| tracing::error!(message_id = "hW9bTq4G", ?error, path =% self.path, "failed to set debug bundle dir mode"))?;
        Ok(())
    }
}

pub async fn create_service_debug_bundle(
    config: &ConfigDebug,
    network_info: &NetworkInfo,
    debug_info: &DebugInfo,
    log_dir: Option<&Utf8Path>,
    peer_uid: Option<PeerUid>,
) -> Result<ServiceDebugBundleHandle, ()> {
    let bundle = ServiceDebugBundleHandle::mkdir().await?;
    tracing::info!(message_id = "kZ6pVw2N", path =% bundle.path, "created service debug bundle dir");
    populate_service_debug_bundle(&bundle.path, config, network_info, debug_info, log_dir).await;
    #[cfg(unix)]
    if let Some(peer_uid) = peer_uid {
        bundle.make_user_readable(peer_uid).await?;
    }
    tracing::info!(
        message_id = "mV5tXn9C",
        path =% bundle.path,
        peer_uid = peer_uid.map(|uid| uid.0),
        "service debug bundle ready"
    );
    Ok(bundle)
}

async fn populate_service_debug_bundle(
    dir: &Utf8Path,
    config: &ConfigDebug,
    network_info: &NetworkInfo,
    debug_info: &DebugInfo,
    log_dir: Option<&Utf8Path>,
) {
    try_write_json_file(dir.join("ne-debug-info.json"), debug_info).await;
    try_write_json_file(dir.join("config-service.json"), config).await;
    try_write_json_file(dir.join("network-service.json"), network_info).await;
    let backend_addrs = config.dns_cache.get(DEFAULT_API_DOMAIN).iter().map(SocketAddr::ip).collect();
    populate_debug_tasks(dir, DebugBundleSide::Service, backend_addrs).await;
    if let Some(log_dir) = log_dir {
        try_copy_dir_contents_recursive(log_dir, &dir.join("logs-service")).await;
        tracing::info!(message_id = "wA8dKp3F", %log_dir, "copied service logs into service debug bundle");
    }
}

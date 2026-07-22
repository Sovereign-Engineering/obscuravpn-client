use serde::{Deserialize, Serialize};

use crate::manager::{Status, VpnStatus};
use uuid::Uuid;

#[serde_with::serde_as]
#[derive(derive_more::Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum NEVPNStatus {
    Invalid,
    Disconnected,
    Connecting,
    Connected,
    Reasserting,
    Disconnecting,
}

impl From<&VpnStatus> for NEVPNStatus {
    fn from(value: &VpnStatus) -> Self {
        match value {
            VpnStatus::Connecting { .. } => Self::Connecting,
            VpnStatus::Connected { .. } => Self::Connected,
            VpnStatus::Disconnected { .. } => Self::Disconnected,
        }
    }
}

#[serde_with::serde_as]
#[derive(derive_more::Debug, Serialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OsStatus {
    pub version: Uuid,
    pub internet_available: bool,
    pub os_vpn_status: NEVPNStatus,
    pub src_version: &'static str,
    pub updater_status: UpdaterStatus,
    pub debug_bundle_status: DebugBundleStatus,
    pub can_send_mail: bool,
    pub service_status: ServiceStatus,
}

#[derive(derive_more::Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ServiceStatus {
    Initializing,
    Healthy(Status),
    Degraded {
        last_status: Option<Status>,
        linux_degradation: LinuxServiceDegradation,
    },
}

#[derive(derive_more::Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum LinuxServiceDegradation {
    Stopped,
    Failed,
    Disabled,
    NotInstalled,
    NoAccess,
    Other,
}

impl OsStatus {
    pub fn set_service_status(&mut self, service_status: ServiceStatus) {
        if self.service_status != service_status {
            self.os_vpn_status = match &service_status {
                ServiceStatus::Healthy(status) => NEVPNStatus::from(&status.vpn_status),
                ServiceStatus::Initializing | ServiceStatus::Degraded { last_status: _, linux_degradation: _ } => NEVPNStatus::Invalid,
            };
            self.service_status = service_status;
            self.version = Uuid::new_v4();
        }
    }
}

impl Default for OsStatus {
    fn default() -> Self {
        Self {
            version: Uuid::new_v4(),
            internet_available: true,
            os_vpn_status: NEVPNStatus::Invalid,
            src_version: option_env!("OBSCURA_VERSION").unwrap_or("v0.0.0-dev"),
            updater_status: Default::default(),
            debug_bundle_status: Default::default(),
            can_send_mail: true,
            service_status: ServiceStatus::Initializing,
        }
    }
}

#[derive(Default, derive_more::Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum UpdaterStatus {
    #[default]
    Uninitiated,
}

#[serde_with::serde_as]
#[derive(derive_more::Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DebugBundleStatus {
    pub in_progress: bool,
    pub latest_path: Option<String>,
    pub in_progress_counter: i64,
}

#[allow(clippy::derivable_impls)]
impl Default for DebugBundleStatus {
    fn default() -> Self {
        Self { in_progress: false, latest_path: None, in_progress_counter: 0 }
    }
}

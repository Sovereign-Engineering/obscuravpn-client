use serde::{Deserialize, Serialize};

use crate::manager::{Status, VpnStatus};
use crate::version::release_version;
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

#[derive(derive_more::Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LinuxServiceDegradation {
    UnitInactive,
    UnitActivating,
    UnitNotInstalled,
    SocketPermissionDenied {
        user: Option<String>,
    },
    VersionMismatch {
        service_version: String,
        app_version: String,
        installed_app_version_differs: Option<bool>,
    },
    Unknown,
}

impl OsStatus {
    pub fn set_debug_bundle_status(&mut self, status: DebugBundleStatus) {
        if self.debug_bundle_status != status {
            self.debug_bundle_status = status;
            self.version = Uuid::new_v4();
        }
    }

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
            src_version: release_version(),
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
#[derive(Default, derive_more::Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DebugBundleStatus {
    pub in_progress: bool,
    pub latest_path: Option<String>,
    pub in_progress_counter: i64,
}

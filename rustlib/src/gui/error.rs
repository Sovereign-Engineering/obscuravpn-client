use obscuravpn_client::linux::ipc::LinuxIpcError;
use obscuravpn_client::manager_cmd::ManagerCmdErrorCode;

#[derive(Debug, Clone, Copy, strum::IntoStaticStr)]
#[strum(prefix = "linuxIpc-", serialize_all = "camelCase")]
pub enum LinuxIpcErrorCode {
    InsufficientPermissions,
    NoListener,
    VersionMismatch,
}

impl LinuxIpcErrorCode {
    pub fn as_static_str(&self) -> &'static str {
        self.into()
    }
}

#[derive(Debug, Clone, Copy, strum::IntoStaticStr)]
#[strum(prefix = "linuxFix-", serialize_all = "camelCase")]
pub enum LinuxFixErrorCode {
    PkexecUnavailable,
    AuthorizationDismissed,
    AuthorizationDenied,
    UsernameUnknown,
    AddOperatorFailed,
    ServiceRestartFailed,
    ServiceEnableAndRestartFailed,
    ServiceStartFailed,
    ServiceStartTimeout,
}

impl LinuxFixErrorCode {
    pub fn as_static_str(&self) -> &'static str {
        self.into()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LinuxErrorCode {
    Other,
    DebugBundleInProgress,
    ManagerCmd(ManagerCmdErrorCode),
    Ipc(LinuxIpcErrorCode),
    Fix(LinuxFixErrorCode),
}

impl LinuxErrorCode {
    pub fn as_static_str(&self) -> &'static str {
        match self {
            Self::Other => "other",
            Self::DebugBundleInProgress => "debugBundleInProgress",
            Self::ManagerCmd(code) => code.as_static_str(),
            Self::Ipc(code) => code.as_static_str(),
            Self::Fix(code) => code.as_static_str(),
        }
    }
}

impl From<ManagerCmdErrorCode> for LinuxErrorCode {
    fn from(code: ManagerCmdErrorCode) -> Self {
        Self::ManagerCmd(code)
    }
}

impl From<&LinuxIpcError> for LinuxErrorCode {
    fn from(error: &LinuxIpcError) -> Self {
        match error {
            LinuxIpcError::InsufficientPermissions => Self::Ipc(LinuxIpcErrorCode::InsufficientPermissions),
            LinuxIpcError::NoListener => Self::Ipc(LinuxIpcErrorCode::NoListener),
            LinuxIpcError::VersionMismatch { service_version: _, app_version: _ } => Self::Ipc(LinuxIpcErrorCode::VersionMismatch),
            LinuxIpcError::Other => Self::Other,
        }
    }
}

impl From<LinuxFixErrorCode> for LinuxErrorCode {
    fn from(code: LinuxFixErrorCode) -> Self {
        Self::Fix(code)
    }
}

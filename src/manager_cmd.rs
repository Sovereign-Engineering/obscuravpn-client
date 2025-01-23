// Command interface for commands, whose arguments and return values can be serialized and deserialized. You should usually prefer other methods unless you are implementing an FFI interface. All commands map more or less directly to another method.

use obscuravpn_api::{
    cmd::{ApiErrorKind, Cmd, GetAccountInfo, ListExits2},
    ClientError,
};
use serde::{Deserialize, Serialize};
use strum::IntoStaticStr;
use uuid::Uuid;

use crate::config::PinnedLocation;
use crate::manager::{Manager, ManagerTrafficStats, Status};
use crate::{config::ConfigSaveError, errors::ApiError};

/// High-level json command error codes, which are actionable for frontends.
/// Actionable means any of:
/// - Useful to trigger specific frontend behavior (e.g. control flow branches)
/// - Correlates with specific error messages shown to users
///
/// All remaining errors are mapped to the `Other` variant.
/// Make sure `obscura-ui/src/translations/en.json` contains an entry for each variant.
///
/// Do not use outside of code processing `ManagerCmd` processing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, IntoStaticStr, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[strum(serialize_all = "camelCase")]
pub enum ManagerCmdErrorCode {
    ApiError,
    ApiNoLongerSupported,
    ApiRateLimitExceeded,
    ApiSignupLimitExceeded,
    ConfigSaveError,
    Other,
}

impl From<&ConfigSaveError> for ManagerCmdErrorCode {
    fn from(err: &ConfigSaveError) -> Self {
        tracing::info!("deriving json cmd error code for {}", &err);
        Self::ConfigSaveError
    }
}

impl From<ApiError> for ManagerCmdErrorCode {
    fn from(err: ApiError) -> Self {
        (&err).into()
    }
}

impl From<&ApiError> for ManagerCmdErrorCode {
    fn from(err: &ApiError) -> Self {
        tracing::info!("deriving json cmd error code for {}", &err);
        match err {
            ApiError::NoAccountId => Self::ApiError,
            ApiError::ApiClient(err) => match err {
                ClientError::ApiError(err) => match err.body.error {
                    ApiErrorKind::NoLongerSupported {} => Self::ApiNoLongerSupported,
                    ApiErrorKind::RateLimitExceeded {} => Self::ApiRateLimitExceeded,
                    ApiErrorKind::SignupLimitExceeded {} => Self::ApiSignupLimitExceeded,
                    ApiErrorKind::InternalError {}
                    | ApiErrorKind::AccountExpired {}
                    | ApiErrorKind::BadRequest {}
                    | ApiErrorKind::MissingOrInvalidAuthToken {}
                    | ApiErrorKind::NoApiRoute {}
                    | ApiErrorKind::NoMatchingExit {}
                    | ApiErrorKind::TunnelLimitExceeded {}
                    | ApiErrorKind::WgKeyRotationRequired {}
                    | ApiErrorKind::Unknown(_) => Self::ApiError,
                },
                ClientError::ProtocolError(_) | ClientError::Other(_) => Self::ApiError,
            },
            ApiError::ConfigSave(err) => err.into(),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ManagerCmd {
    Ping {},
    GetTrafficStats {},
    SetPinnedExits { exits: Vec<PinnedLocation> },
    Login { account_id: String, validate: bool },
    Logout {},
    SetApiUrl { url: Option<String> },
    ApiGetAccountInfo {},
    ApiListExit {},
    GetStatus { known_version: Option<Uuid> },
    SetInNewAccountFlow { value: bool },
}

#[allow(clippy::large_enum_variant)]
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum ManagerCmdOk {
    Empty,
    GetTrafficStats(ManagerTrafficStats),
    ApiListExit(<ListExits2 as Cmd>::Output),
    ApiGetAccountInfo(<GetAccountInfo as Cmd>::Output),
    GetStatus(Status),
}

impl ManagerCmd {
    pub(super) async fn run(self, manager: &Manager) -> Result<ManagerCmdOk, ManagerCmdErrorCode> {
        match self {
            ManagerCmd::GetTrafficStats {} => Ok(ManagerCmdOk::GetTrafficStats(manager.traffic_stats())),
            ManagerCmd::SetPinnedExits { exits } => match manager.set_pinned_exits(exits) {
                Ok(()) => Ok(ManagerCmdOk::Empty),
                Err(err) => Err((&err).into()),
            },
            ManagerCmd::Login { account_id, validate } => match manager.login(account_id, validate).await {
                Ok(()) => Ok(ManagerCmdOk::Empty),
                Err(err) => Err((&err).into()),
            },
            ManagerCmd::Logout {} => match manager.logout() {
                Ok(()) => Ok(ManagerCmdOk::Empty),
                Err(err) => Err((&err).into()),
            },
            ManagerCmd::ApiGetAccountInfo {} => match manager.get_account_info().await {
                Ok(account_info) => Ok(ManagerCmdOk::ApiGetAccountInfo(account_info)),
                Err(error) => Err((&error).into()),
            },
            ManagerCmd::ApiListExit {} => match manager.list_exits().await {
                Ok(exit_list) => Ok(ManagerCmdOk::ApiListExit(exit_list)),
                Err(error) => Err((&error).into()),
            },
            ManagerCmd::GetStatus { known_version } => match manager.subscribe().wait_for(|s| Some(s.version) != known_version).await {
                Ok(status) => Ok(ManagerCmdOk::GetStatus(status.clone())),
                Err(_err) => {
                    tracing::error!("status subscription channel closed");
                    Err(ManagerCmdErrorCode::Other)
                }
            },
            ManagerCmd::Ping {} => Ok(ManagerCmdOk::Empty),
            ManagerCmd::SetInNewAccountFlow { value } => match manager.set_in_new_account_flow(value) {
                Ok(()) => Ok(ManagerCmdOk::Empty),
                Err(err) => Err((&err).into()),
            },
            ManagerCmd::SetApiUrl { url } => match manager.set_api_url(url) {
                Ok(()) => Ok(ManagerCmdOk::Empty),
                Err(err) => Err((&err).into()),
            },
        }
    }
}

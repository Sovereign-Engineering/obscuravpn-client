// Command interface for commands, whose arguments and return values can be serialized and deserialized. You should usually prefer other methods unless you are implementing an FFI interface. All commands map more or less directly to another method.

use std::{sync::Arc, thread::sleep, time::Duration};

use base64::prelude::*;
use obscuravpn_api::{
    ClientError,
    cmd::{ApiErrorKind, AppleAssociateAccountOutput, AppleCreateAppAccountTokenOutput, ApplePollSubscriptionOutput, ExitList},
    types::{AccountId, AccountInfo},
};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use strum::IntoStaticStr;
use tokio::spawn;
use uuid::Uuid;

use crate::{
    cached_value::CachedValue,
    manager::{DebugInfo, Manager, ManagerTrafficStats, Status},
};
use crate::{config::ConfigSaveError, errors::ApiError};
use crate::{config::PinnedLocation, manager::TunnelArgs};

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
    ApiUnreachable,
    ConfigSaveError,
    Other,
    TunnelInactive,
}

impl From<&ConfigSaveError> for ManagerCmdErrorCode {
    fn from(err: &ConfigSaveError) -> Self {
        tracing::info!("deriving json cmd error code for {}", &err);
        Self::ConfigSaveError
    }
}

impl From<&ApiError> for ManagerCmdErrorCode {
    fn from(err: &ApiError) -> Self {
        tracing::info!("deriving json cmd error code for {}", &err);
        match err {
            ApiError::ApiClient(err) => match err {
                ClientError::ApiError(err) => match err.body.error {
                    ApiErrorKind::NoLongerSupported {} => Self::ApiNoLongerSupported,
                    ApiErrorKind::RateLimitExceeded {} => Self::ApiRateLimitExceeded,
                    ApiErrorKind::SignupLimitExceeded {} => Self::ApiSignupLimitExceeded,
                    ApiErrorKind::AccountExpired {}
                    | ApiErrorKind::AlreadyExists {}
                    | ApiErrorKind::BadRequest {}
                    | ApiErrorKind::InternalError {}
                    | ApiErrorKind::MissingOrInvalidAuthToken {}
                    | ApiErrorKind::NoApiRoute {}
                    | ApiErrorKind::NoMatchingExit {}
                    | ApiErrorKind::SaleNotFound {}
                    | ApiErrorKind::TunnelLimitExceeded {}
                    | ApiErrorKind::WgKeyRotationRequired {}
                    | ApiErrorKind::Unknown(_) => Self::ApiError,
                },
                ClientError::RequestExecError(_) => Self::ApiUnreachable,
                ClientError::ResponseTooLarge | ClientError::InvalidHeaderValue | ClientError::Other(_) | ClientError::ProtocolError(_) => {
                    Self::ApiError
                }
            },
            ApiError::ConfigSave(err) => err.into(),
            ApiError::NoAccountId => Self::ApiError,
        }
    }
}

// Keep synchronized with ../../apple/shared/NetworkExtensionIpc.swift
#[serde_with::serde_as]
#[derive(derive_more::Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ManagerCmd {
    ApiAppleAssociateAccount {
        app_transaction_jws: String,
    },
    ApiAppleCreateAppAccountToken {},
    ApiApplePollSubscription {
        original_transaction_id: String,
    },
    ApiGetAccountInfo {},
    GetDebugInfo {},
    GetExitList {
        #[debug("{:?}", known_version.as_ref().map(|b| BASE64_STANDARD.encode(b)))]
        #[serde_as(as = "Option<serde_with::base64::Base64>")]
        known_version: Option<Vec<u8>>,
    },
    GetLogDir {},
    GetStatus {
        known_version: Option<Uuid>,
    },
    GetTrafficStats {},
    TerminateProcess {},
    Login {
        account_id: AccountId,
        validate: bool,
    },
    Logout {},
    Ping {},
    RefreshExitList {
        #[serde_as(as = "serde_with::DurationMilliSeconds")]
        freshness: Duration,
    },
    RotateWgKey {},
    SetApiHostAlternate {
        host: Option<String>,
    },
    SetApiUrl {
        url: Option<String>,
    },
    SetAutoConnect {
        enable: bool,
    },
    SetFeatureFlag {
        flag: String,
        active: bool,
    },
    SetInNewAccountFlow {
        value: bool,
    },
    SetPinnedExits {
        exits: Vec<PinnedLocation>,
    },
    SetSniRelay {
        host: Option<String>,
    },
    SetTunnelArgs {
        args: Option<TunnelArgs>,
        allow_activation: bool,
    },
    ToggleForceTcpTlsRelayTransport {},
}

#[derive(Debug, derive_more::From, Serialize)]
#[serde(untagged)]
pub enum ManagerCmdOk {
    #[from]
    ApiAppleAssociateAccount(AppleAssociateAccountOutput),
    #[from]
    ApiAppleCreateAppAccountToken(AppleCreateAppAccountTokenOutput),
    #[from]
    ApiApplePollSubscription(ApplePollSubscriptionOutput),
    #[from]
    ApiGetAccountInfo(AccountInfo),
    Empty,
    GetDebugInfo(DebugInfo),
    GetExitList(CachedValue<Arc<ExitList>>),
    GetLogDir(String),
    GetStatus(Status),
    GetTrafficStats(ManagerTrafficStats),
}

impl From<()> for ManagerCmdOk {
    fn from((): ()) -> Self {
        Self::Empty
    }
}

fn map_result<T, E>(result: Result<T, E>) -> Result<ManagerCmdOk, ManagerCmdErrorCode>
where
    T: Into<ManagerCmdOk>,
    for<'r> &'r E: Into<ManagerCmdErrorCode>,
{
    result.map(Into::into).map_err(|err| (&err).into())
}

impl ManagerCmd {
    pub(super) async fn run(self, manager: &Manager) -> Result<ManagerCmdOk, ManagerCmdErrorCode> {
        match self {
            Self::ApiAppleAssociateAccount { app_transaction_jws } => map_result(manager.apple_associate_account(app_transaction_jws).await),
            Self::ApiAppleCreateAppAccountToken {} => map_result(manager.apple_create_app_account_token().await),
            Self::ApiApplePollSubscription { original_transaction_id } => map_result(manager.apple_poll_subscription(original_transaction_id).await),
            Self::ApiGetAccountInfo {} => map_result(manager.get_account_info().await),
            Self::SetFeatureFlag { flag, active } => map_result(manager.set_feature_flag(&flag, active)),
            Self::GetDebugInfo {} => Ok(ManagerCmdOk::GetDebugInfo(manager.get_debug_info())),
            Self::GetExitList { known_version } => {
                let mut recv = manager.subscribe_exit_list();
                let res = recv
                    .wait_for(|exits| exits.as_ref().is_some_and(|e| Some(e.version()) != known_version.as_deref()))
                    .await
                    .map_err(|error| {
                        tracing::error!(?error, message_id = "ahcieM1h", "exit list subscription channel closed: {}", error,);
                        ManagerCmdErrorCode::Other
                    })?;
                let res = res.as_ref().unwrap();
                Ok(ManagerCmdOk::GetExitList(CachedValue {
                    version: res.version().to_vec(),
                    last_updated: res.last_updated,
                    value: res.value.clone(),
                }))
            }
            Self::GetLogDir {} => manager.get_log_dir().map(ManagerCmdOk::GetLogDir).map_err(|error| {
                tracing::error!(?error, message_id = "VtC1zLGx", "failed to get log dir");
                ManagerCmdErrorCode::Other
            }),
            Self::GetStatus { known_version } => manager
                .subscribe()
                .wait_for(|s| Some(s.version) != known_version)
                .await
                .map(|status| ManagerCmdOk::GetStatus(status.clone()))
                .map_err(|_err| {
                    tracing::error!("status subscription channel closed");
                    ManagerCmdErrorCode::Other
                }),
            Self::GetTrafficStats {} => Ok(ManagerCmdOk::GetTrafficStats(manager.traffic_stats())),
            Self::TerminateProcess {} => {
                const WAIT: Duration = Duration::from_secs(3);
                tracing::error!(message_id = "i5BA5bOA", "received termination command, exiting in {}ms", WAIT.as_millis());
                spawn(async {
                    sleep(Duration::from_secs(3));
                    tracing::error!(message_id = "eCoVnCI6", "executing scheduled termination");
                    std::process::exit(1);
                });
                Ok(ManagerCmdOk::Empty)
            }
            Self::Login { account_id, validate } => map_result(manager.login(account_id, validate).await),
            Self::Logout {} => map_result(manager.logout()),
            Self::Ping {} => Ok(ManagerCmdOk::Empty),
            Self::RefreshExitList { freshness } => map_result(manager.maybe_update_exits(freshness).await),
            Self::RotateWgKey {} => map_result(manager.rotate_wg_key()),
            Self::SetAutoConnect { enable } => map_result(manager.set_auto_connect(enable)),
            Self::SetApiHostAlternate { host } => map_result(manager.set_api_host_alternate(host)),
            Self::SetApiUrl { url } => map_result(manager.set_api_url(url)),
            Self::SetInNewAccountFlow { value } => map_result(manager.set_in_new_account_flow(value)),
            Self::SetPinnedExits { exits } => map_result(manager.set_pinned_exits(exits)),
            Self::SetSniRelay { host } => map_result(manager.set_sni_relay(host)),
            Self::SetTunnelArgs { args, allow_activation } => manager
                .set_target_state(args, allow_activation)
                .map(Into::into)
                .map_err(|()| ManagerCmdErrorCode::TunnelInactive),
            Self::ToggleForceTcpTlsRelayTransport {} => map_result(manager.toggle_force_tcp_tls_relay_transport()),
        }
    }
}

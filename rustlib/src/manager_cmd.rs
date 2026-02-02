// Command interface for commands, whose arguments and return values can be serialized and deserialized. You should usually prefer other methods unless you are implementing an FFI interface. All commands map more or less directly to another method.

use std::{sync::Arc, time::Duration};

use base64::prelude::*;
use obscuravpn_api::{
    ClientError,
    cmd::{ApiErrorKind, AppleAssociateAccountOutput, DeleteAccountOutput, ExitList},
    types::{AccountId, AccountInfo},
};
use serde::{Deserialize, Serialize};
use serde_with::serde_as;
use strum::IntoStaticStr;
use tokio::spawn;
use uuid::Uuid;

use crate::client_state::ClientStateHandle;
use crate::errors::ApiError;
use crate::errors::{ConfigDirty, ConfigDirtyOrApiError};
use crate::network_config::DnsContentBlock;
use crate::{
    cached_value::CachedValue,
    manager::{DebugInfo, Manager, ManagerTrafficStats, Status},
};
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
    ApiInvalidAccountId,
    ApiNoLongerSupported,
    ApiRateLimitExceeded,
    ApiSignupLimitExceeded,
    ApiUnreachable,
    ConfigSaveError,
    Other,
}

impl ManagerCmdErrorCode {
    pub fn as_static_str(&self) -> &'static str {
        self.into()
    }
}

impl From<&ConfigDirty> for ManagerCmdErrorCode {
    fn from(error: &ConfigDirty) -> Self {
        tracing::info!(message_id = "7YMEQ3ac", ?error, "deriving json cmd error code for: {}", &error);
        Self::ConfigSaveError
    }
}

impl From<&ConfigDirtyOrApiError> for ManagerCmdErrorCode {
    fn from(error: &ConfigDirtyOrApiError) -> Self {
        tracing::info!(message_id = "7oPu26ad", ?error, "deriving json cmd error code for: {}", &error);
        match error {
            ConfigDirtyOrApiError::ApiError(error) => error.into(),
            ConfigDirtyOrApiError::ConfigDirty(error) => error.into(),
        }
    }
}

impl From<&ApiError> for ManagerCmdErrorCode {
    fn from(error: &ApiError) -> Self {
        tracing::info!(message_id = "ch2a5Sp5", ?error, "deriving json cmd error code for: {}", &error);
        match error {
            ApiError::ApiClient(err) => match err {
                ClientError::ApiError(err) => match err.body.error {
                    ApiErrorKind::NoLongerSupported {} => Self::ApiNoLongerSupported,
                    ApiErrorKind::RateLimitExceeded {} => Self::ApiRateLimitExceeded,
                    ApiErrorKind::SignupLimitExceeded {} => Self::ApiSignupLimitExceeded,
                    ApiErrorKind::InvalidAccountId {} => Self::ApiInvalidAccountId,
                    ApiErrorKind::AccountExpired {}
                    | ApiErrorKind::AlreadyExists {}
                    | ApiErrorKind::AlreadyReferred {}
                    | ApiErrorKind::BadRequest {}
                    | ApiErrorKind::IneligibleForReferral {}
                    | ApiErrorKind::InternalError {}
                    | ApiErrorKind::InvalidReferralCode {}
                    | ApiErrorKind::MissingOrInvalidAuthToken {}
                    | ApiErrorKind::MiscUnauthorized {}
                    | ApiErrorKind::MoneroTopUpNotFound {}
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
    ApiDeleteAccount {},
    ApiGetAccountInfo {},
    CreateDebugArchive {
        user_feedback: Option<String>,
    },
    GetDebugInfo {},
    GetExitList {
        #[debug("{:?}", known_version.as_ref().map(|b| BASE64_STANDARD.encode(b)))]
        #[serde_as(as = "Option<serde_with::base64::Base64>")]
        known_version: Option<Vec<u8>>,
    },
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
    SetDnsContentBlock {
        value: DnsContentBlock,
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
        active: Option<bool>,
    },
    SetUseSystemDns {
        enable: bool,
    },
}

#[derive(Debug, derive_more::From, Serialize)]
#[serde(untagged)]
pub enum ManagerCmdOk {
    #[from]
    ApiAppleAssociateAccount(AppleAssociateAccountOutput),
    #[from]
    ApiDeleteAccount(DeleteAccountOutput),
    #[from]
    ApiGetAccountInfo(AccountInfo),
    CreateDebugArchive(String),
    Empty,
    GetDebugInfo(DebugInfo),
    GetExitList(CachedValue<Arc<ExitList>>),
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
    pub fn from_json(json_cmd: &[u8]) -> Result<Self, ManagerCmdErrorCode> {
        // apple frameworks log IPC message SHA1
        let hash = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, json_cmd);

        let cmd: ManagerCmd = serde_json::from_slice(json_cmd).map_err(|error| {
            tracing::error!(
                ?error,
                cmd =? String::from_utf8_lossy(json_cmd),
                hash =? hash,
                message_id = "ahsh9Aec",
                "could not decode json command: {error}",
            );
            ManagerCmdErrorCode::Other
        })?;

        tracing::info!(
            cmd = format!("{:#?}", cmd),
            hash =? hash,
            message_id = "JumahFi5",
            "decoded json cmd",
        );

        Ok(cmd)
    }

    pub async fn run(self, manager: &Manager) -> Result<ManagerCmdOk, ManagerCmdErrorCode> {
        match self {
            Self::ApiAppleAssociateAccount { app_transaction_jws } => map_result(manager.apple_associate_account(app_transaction_jws).await),
            Self::ApiDeleteAccount {} => map_result(manager.delete_account().await),
            Self::ApiGetAccountInfo {} => map_result(manager.get_account_info().await),
            Self::SetFeatureFlag { flag, active } => manager.run_on_client_state(|c| c.set_feature_flag(&flag, active)),
            Self::CreateDebugArchive { user_feedback } => manager
                .create_debug_archive(user_feedback.as_deref())
                .await
                .map(ManagerCmdOk::CreateDebugArchive)
                .map_err(|error| {
                    tracing::error!(?error, "failed to create debug archive");
                    ManagerCmdErrorCode::Other
                }),
            Self::GetDebugInfo {} => Ok(ManagerCmdOk::GetDebugInfo(manager.get_debug_info())),
            Self::GetExitList { known_version } => manager.get_exit_list(known_version).await.map(ManagerCmdOk::GetExitList),
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
                    tokio::time::sleep(Duration::from_secs(3)).await;
                    tracing::error!(message_id = "eCoVnCI6", "executing scheduled termination");
                    std::process::exit(1);
                });
                Ok(ManagerCmdOk::Empty)
            }
            Self::Login { account_id, validate } => map_result(manager.login(account_id, validate).await),
            Self::Logout {} => map_result(manager.logout()),
            Self::Ping {} => Ok(ManagerCmdOk::Empty),
            Self::RefreshExitList { freshness } => map_result(manager.maybe_update_exits(freshness).await),
            Self::RotateWgKey {} => manager.run_on_client_state(ClientStateHandle::rotate_wg_key),
            Self::SetAutoConnect { enable } => manager.run_on_client_state(|c| c.set_auto_connect(enable)),
            Self::SetApiHostAlternate { host } => manager.run_on_client_state(|c| c.set_api_host_alternate(host)),
            Self::SetApiUrl { url } => manager.run_on_client_state(|c| c.set_api_url(url)),
            Self::SetDnsContentBlock { value } => manager.run_on_client_state(|c| c.set_dns_content_block(value)),
            Self::SetInNewAccountFlow { value } => manager.run_on_client_state(|c| c.set_in_new_account_flow(value)),
            Self::SetPinnedExits { exits } => manager.run_on_client_state(|c| c.set_pinned_exits(exits)),
            Self::SetSniRelay { host } => manager.run_on_client_state(|c| c.set_sni_relay(host)),
            Self::SetTunnelArgs { args, active } => manager.run_on_client_state(|c| c.set_tunnel_target_state(args, active)),
            Self::SetUseSystemDns { enable } => manager.run_on_client_state(|c| c.set_use_system_dns(enable)),
        }
    }
}

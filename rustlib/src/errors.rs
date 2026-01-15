use std::io;
use std::time::Instant;

use obscuravpn_api::{ClientError, cmd::ApiErrorKind};
use serde::{Deserialize, Serialize};
use strum::IntoStaticStr;
use thiserror::Error;

use crate::config::ConfigSaveError;
use crate::quicwg::QuicWgConnectError;

use crate::network_config::NetworkConfigError;

/// High-level connection error codes, which are actionable for frontends.
/// Actionable means any of:
/// - Useful to trigger specific frontend behavior (e.g. control flow branches)
/// - Correlates with specific error messages shown to users
///
/// All remaining errors are mapped to the `Other` variant.
/// Make sure `obscura-ui/src/translations/en.json` contains an entry for each variant.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, IntoStaticStr, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[strum(serialize_all = "camelCase")]
pub enum ConnectErrorCode {
    AccountExpired,
    ApiError,
    ApiRateLimitExceeded,
    ApiUnreachable,
    InvalidAccountId,
    NoInternet,
    NoLongerSupported,
    NoSlotsLeft,
    Other,
}

impl ConnectErrorCode {
    pub fn as_static_str(&self) -> &'static str {
        self.into()
    }
}

impl From<&TunnelConnectError> for ConnectErrorCode {
    fn from(err: &TunnelConnectError) -> Self {
        use ApiErrorKind::*;
        tracing::info!("deriving connect error code for {}", err);
        match err {
            TunnelConnectError::ApiError(err) => match err {
                ApiError::NoAccountId => Self::Other,
                ApiError::ApiClient(err) => match err {
                    ClientError::ApiError(err) => match err.body.error {
                        AccountExpired {} => Self::AccountExpired,
                        InvalidAccountId {} => Self::InvalidAccountId,
                        NoLongerSupported {} => Self::NoLongerSupported,
                        TunnelLimitExceeded {} => Self::NoSlotsLeft,
                        RateLimitExceeded {} => Self::ApiRateLimitExceeded,
                        AlreadyExists {}
                        | AlreadyReferred {}
                        | BadRequest {}
                        | IneligibleForReferral {}
                        | InternalError {}
                        | InvalidReferralCode {}
                        | MiscUnauthorized {}
                        | MissingOrInvalidAuthToken {}
                        | MoneroTopUpNotFound {}
                        | NoApiRoute {}
                        | NoMatchingExit {}
                        | SaleNotFound {}
                        | SignupLimitExceeded {}
                        | WgKeyRotationRequired {}
                        | Unknown(_) => Self::ApiError,
                    },
                    ClientError::RequestExecError(_) => Self::ApiUnreachable,
                    ClientError::ResponseTooLarge | ClientError::InvalidHeaderValue | ClientError::Other(_) | ClientError::ProtocolError(_) => {
                        Self::Other
                    }
                },
                ApiError::ConfigSave(_) => Self::Other,
            },
            TunnelConnectError::NoInternet => Self::NoInternet,
            TunnelConnectError::NetworkConfig(_)
            | TunnelConnectError::NoExit
            | TunnelConnectError::TunnelConnect(_)
            | TunnelConnectError::InvalidTunnelId
            | TunnelConnectError::UnexpectedRelay
            | TunnelConnectError::UnexpectedTunnelKind
            | TunnelConnectError::UnexpectedInternalTunnelLifecycleState
            | TunnelConnectError::RelaySelection(_)
            | TunnelConnectError::ConfigSave(_) => Self::Other,
        }
    }
}

#[derive(Debug, Error)]
pub enum TunnelConnectError {
    #[error("tunnel creation: {0}")]
    ApiError(#[from] ApiError),
    #[error("failed to save config file")]
    ConfigSave(#[from] ConfigSaveError),
    #[error("api returned invalid tunnel id")]
    InvalidTunnelId,
    #[error("could not construct network config: {0}")]
    NetworkConfig(#[from] NetworkConfigError),
    #[error("No matching exit.")]
    NoExit,
    #[error("No internet.")]
    NoInternet,
    #[error("relay selection failed: {0}")]
    RelaySelection(#[from] RelaySelectionError),
    #[error("tunnel connect: {0}")]
    TunnelConnect(#[from] QuicWgConnectError),
    #[error("tunnel is in unexpected internal lifecycle state")]
    UnexpectedInternalTunnelLifecycleState,
    #[error("api returned unexpected relay")]
    UnexpectedRelay,
    #[error("api returned unexpected tunnel kind")]
    UnexpectedTunnelKind,
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error(transparent)]
    ApiClient(#[from] ClientError),
    #[error(transparent)]
    ConfigSave(#[from] ConfigSaveError),
    #[error("no account id")]
    NoAccountId,
}

impl ApiError {
    pub fn api_error_kind(&self) -> Option<&obscuravpn_api::cmd::ApiErrorKind> {
        if let Self::ApiClient(ClientError::ApiError(error)) = self {
            return Some(&error.body.error);
        }
        None
    }
}

#[derive(Debug, Error)]
pub enum RelaySelectionError {
    #[error("all relay connections failed")]
    NoSuccess,
    #[error("quic setup: {0}")]
    QuicSetup(anyhow::Error),
    #[error("udp socket setup: {0}")]
    UdpSetup(io::Error),
}

#[derive(Debug, Error)]
pub struct ErrorAt<T: std::error::Error> {
    pub error: T,
    pub at: Instant,
}

impl<T: std::error::Error> From<T> for ErrorAt<T> {
    fn from(error: T) -> Self {
        Self { error, at: Instant::now() }
    }
}

use std::io;
use std::time::Instant;

use obscuravpn_api::{cmd::ApiErrorKind, ClientError};
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
    InvalidAccountId,
    NoSlotsLeft,
    AccountExpired,
    ApiRateLimitExceeded,
    ApiError,
    NoLongerSupported,
    Other,
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
                        NoLongerSupported {} => Self::NoLongerSupported,
                        TunnelLimitExceeded {} => Self::NoSlotsLeft,
                        RateLimitExceeded {} => Self::ApiRateLimitExceeded,
                        AlreadyExists {}
                        | BadRequest {}
                        | InternalError {}
                        | MissingOrInvalidAuthToken {}
                        | NoApiRoute {}
                        | NoMatchingExit {}
                        | SignupLimitExceeded {}
                        | WgKeyRotationRequired {}
                        | Unknown(_) => Self::ApiError,
                    },
                    ClientError::ProtocolError(_) | ClientError::Other(_) => Self::Other,
                },
                ApiError::ConfigSave(_) => Self::Other,
            },
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
    #[error("could not construct network config: {0}")]
    NetworkConfig(#[from] NetworkConfigError),
    #[error("tunnel connect: {0}")]
    TunnelConnect(#[from] QuicWgConnectError),
    #[error("relay selection failed: {0}")]
    RelaySelection(#[from] RelaySelectionError),
    #[error("api returned invalid tunnel id")]
    InvalidTunnelId,
    #[error("No matching exit.")]
    NoExit,
    #[error("api returned and unexpected relay")]
    UnexpectedRelay,
    #[error("tunnel is in unexpected internal lifecycle state")]
    UnexpectedInternalTunnelLifecycleState,
    #[error("api returned unexpected tunnel kind")]
    UnexpectedTunnelKind,
    #[error("failed to save config file")]
    ConfigSave(#[from] ConfigSaveError),
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("no account id")]
    NoAccountId,
    #[error(transparent)]
    ApiClient(#[from] ClientError),
    #[error(transparent)]
    ConfigSave(#[from] ConfigSaveError),
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
    #[error("udp socket setup: {0}")]
    UdpSetup(io::Error),
    #[error("quic setup: {0}")]
    QuicSetup(anyhow::Error),
    #[error("all relay connections failed")]
    NoSuccess,
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

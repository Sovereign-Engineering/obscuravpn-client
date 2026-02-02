use obscuravpn_client::manager_cmd::ManagerCmdErrorCode;

#[derive(thiserror::Error, Debug)]
pub enum ClientError {
    #[error("The Obscura API is unreachable.")]
    ApiUnreachable,
    #[error("Insufficient permissions to connect to service. Use sudo or add the user to the obscura group.")]
    InsufficientPermissions,
    #[error("Unexpected error. Details: {0:#}")]
    Unexpected(#[from] anyhow::Error),
    #[error("The Obscura VPN service is not running.")]
    NoService,
    #[error("Malformed account ID.")]
    MalformedAccountId,
}

impl From<ManagerCmdErrorCode> for ClientError {
    fn from(error: ManagerCmdErrorCode) -> ClientError {
        match error {
            ManagerCmdErrorCode::ApiInvalidAccountId => ClientError::MalformedAccountId,
            ManagerCmdErrorCode::ApiUnreachable => ClientError::ApiUnreachable,
            ManagerCmdErrorCode::ApiError
            | ManagerCmdErrorCode::ApiNoLongerSupported
            | ManagerCmdErrorCode::ApiRateLimitExceeded
            | ManagerCmdErrorCode::ApiSignupLimitExceeded
            | ManagerCmdErrorCode::ConfigSaveError
            | ManagerCmdErrorCode::Other => anyhow::Error::msg(error.as_static_str()).into(),
        }
    }
}

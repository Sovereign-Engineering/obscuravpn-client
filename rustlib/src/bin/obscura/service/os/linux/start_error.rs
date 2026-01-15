#[derive(thiserror::Error, Debug)]
pub enum ServiceStartError {
    #[error("Insufficient permissions to start service. Usually requires root.")]
    InsufficientPermissions,
    #[error("Another instance of Obscura VPN is already running.")]
    AlreadyRunning,
    #[error("No supported DNS manager detected.")]
    NoDnsManager,
    #[error("Unexpected error. Details: {0}")]
    Unexpected(#[from] anyhow::Error),
}

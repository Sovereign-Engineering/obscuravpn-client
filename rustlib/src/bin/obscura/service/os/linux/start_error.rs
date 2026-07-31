#[derive(thiserror::Error, Debug)]
pub enum LinuxServiceStartError {
    #[error("Insufficient permissions to start service. Usually requires root.")]
    InsufficientPermissions,
    #[error("Another instance of Obscura VPN is already running.")]
    AlreadyRunning,
    #[error("No supported DNS manager detected.")]
    NoDnsManager,
    #[error("Failed to set up nftables.")]
    NftablesSetup,
    #[error("Unexpected error. Details: {0}")]
    Unexpected(#[from] anyhow::Error),
}

use std::path::PathBuf;

#[derive(thiserror::Error, Debug)]
pub enum WindowsServiceStartError {
    #[error("Failed to get current exe path: {0}")]
    CurrentExePath(#[source] std::io::Error),
    #[error("wintun.dll hash mismatch (location {dll_path}, expected {expected}, got {actual})")]
    WintunDllHashMismatch { dll_path: PathBuf, expected: String, actual: String },
    #[error("Failed to read wintun.dll for hash verification: {0}")]
    WintunDllRead(#[source] std::io::Error),
    #[error("Failed to load wintun dll: {0}")]
    LoadWintunDll(#[source] wintun::Error),
    #[error("Failed to create wintun adapter: {0}")]
    CreateWintunAdapter(#[source] wintun::Error),
    #[error("Failed to start wintun session: {0}")]
    StartWintunSession(#[source] wintun::Error),
    #[error("Unexpected error. Details: {0}")]
    Unexpected(#[from] anyhow::Error),
}

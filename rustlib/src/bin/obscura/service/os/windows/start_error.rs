#[derive(thiserror::Error, Debug)]
pub enum WindowsServiceStartError {
    #[error("Unexpected error. Details: {0}")]
    Unexpected(#[from] anyhow::Error),
}

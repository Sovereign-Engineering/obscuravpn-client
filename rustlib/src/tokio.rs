pub struct AbortOnDrop(pub tokio::task::AbortHandle);

impl Drop for AbortOnDrop {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl From<tokio::task::AbortHandle> for AbortOnDrop {
    fn from(handle: tokio::task::AbortHandle) -> Self {
        AbortOnDrop(handle)
    }
}

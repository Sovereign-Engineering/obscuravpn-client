use crate::service::os::linux::start_error::ServiceStartError;
use std::fs::{File, TryLockError};
use std::io::ErrorKind;

const LOCK_PATH: &str = "/run/obscura.lock";

pub struct ServiceLock {
    _file: File,
}

impl ServiceLock {
    pub fn new() -> Result<Self, ServiceStartError> {
        let mut options = File::options();
        options.create(true).write(true).read(true);
        let file = options.open(LOCK_PATH).map_err(|error| {
            tracing::error!(message_id = "muFNujy4", ?error, "failed to create or open lock file: {error}");
            match error.kind() {
                ErrorKind::PermissionDenied => ServiceStartError::InsufficientPermissions,
                _ => anyhow::Error::new(error).context("failed to create or open lock file").into(),
            }
        })?;
        file.try_lock().map_err(|error| {
            tracing::error!(message_id = "wwkKzjFi", ?error, "failed to take exclusive lock on lock file: {error}");
            match error {
                TryLockError::WouldBlock => ServiceStartError::AlreadyRunning,
                error => anyhow::Error::new(error).context("failed to take exclusive lock on lock file").into(),
            }
        })?;
        Ok(Self { _file: file })
    }
}

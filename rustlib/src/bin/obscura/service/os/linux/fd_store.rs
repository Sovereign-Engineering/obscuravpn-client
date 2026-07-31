use sd_notify::NotifyState;
use std::os::fd::{AsRawFd, BorrowedFd, FromRawFd, OwnedFd, RawFd};

pub struct FdStore {
    entries: Vec<Entry>,
}

struct Entry {
    name: String,
    raw_fd: RawFd,
    fd: Option<OwnedFd>,
}

impl FdStore {
    pub fn take_from_systemd() -> Self {
        let mut entries = Vec::new();
        // SAFETY: Runs once during startup and nothing else in the process reads or writes the LISTEN_* environment variables.
        match unsafe { sd_notify::listen_fds_with_names_and_unset_env() } {
            Ok(stored_fds) => {
                for (raw_fd, name) in stored_fds {
                    tracing::info!(message_id = "vG5xMp1S", raw_fd, name, "took fd from systemd fd store");
                    // SAFETY: Per the sd_listen_fds contract (https://www.freedesktop.org/software/systemd/man/latest/sd_listen_fds.html) these fds are open and owned by us, and this is the only place that takes ownership of them.
                    entries.push(Entry { name, raw_fd, fd: Some(unsafe { OwnedFd::from_raw_fd(raw_fd) }) });
                }
            }
            Err(error) => {
                tracing::error!(message_id = "pY7cKu2D", ?error, "failed to receive fds from systemd fd store");
            }
        }
        tracing::info!(message_id = "wA4nRk7T", count = entries.len(), "took fds from systemd fd store");
        Self { entries }
    }

    #[must_use]
    pub fn take(&mut self, name: &str) -> Option<OwnedFd> {
        if self.entries.iter().filter(|entry| entry.name == name).count() > 1 {
            tracing::error!(message_id = "kR4vXn7D", name, "refusing to hand out fd with duplicated name");
            return None;
        }
        let entry = self.entries.iter_mut().find(|entry| entry.name == name)?;
        let fd = entry.fd.take();
        if fd.is_none() {
            tracing::error!(
                message_id = "nY5wKd8Q",
                name,
                raw_fd = entry.raw_fd,
                "refusing to hand out stored fd twice"
            );
        }
        fd
    }

    /// Stores `fd`, replacing any fds stored under the same name. The replacement is not atomic: a crash can lose the old fds before the new one is stored.
    pub fn remove_old_and_store(&mut self, name: &str, fd: BorrowedFd<'_>) {
        let raw_fd = fd.as_raw_fd();
        self.entries.retain(|entry| entry.name != name);
        remove(name);
        match sd_notify::notify_with_fds(&[NotifyState::FdStore, NotifyState::FdName(name)], &[fd]) {
            Ok(()) => {
                tracing::info!(message_id = "cJ3fWq8M", name, raw_fd, "stored fd in systemd fd store");
                self.entries.push(Entry { name: name.into(), raw_fd, fd: None });
            }
            Err(error) => tracing::error!(message_id = "kD6pXw2R", ?error, name, "failed to store fd in systemd fd store"),
        }
    }

    pub fn remove_unclaimed(&mut self) {
        self.entries.retain(|Entry { name, raw_fd, fd }| match fd {
            Some(_) => {
                tracing::error!(message_id = "uQ9tBv4N", raw_fd, name, "removing extra stored fd");
                remove(name);
                false
            }
            None => true,
        });
    }
}

fn remove(name: &str) {
    if let Err(error) = sd_notify::notify(&[NotifyState::FdStoreRemove, NotifyState::FdName(name)]) {
        tracing::error!(message_id = "sQ4jHn7Y", ?error, name, "failed to remove fds from systemd fd store");
    }
}

pub mod debug_bundle;
pub mod exit_list_watch;
pub mod file_manager;
pub mod ipc;
pub mod status;
pub mod status_watch;
pub mod systemd;
pub mod tray;

pub fn argv0() -> Option<std::path::PathBuf> {
    std::env::args_os().next().map(std::path::PathBuf::from)
}

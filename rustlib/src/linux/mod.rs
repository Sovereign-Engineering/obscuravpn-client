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

pub async fn current_user_name() -> Option<String> {
    let user = tokio::task::spawn_blocking(|| nix::unistd::User::from_uid(nix::unistd::getuid()))
        .await
        .map_err(|error| tracing::error!(message_id = "bW5nTk8D", %error, "task resolving current user failed"))
        .ok()?
        .map_err(|error| tracing::error!(message_id = "rV2mXs7J", %error, "failed to resolve current user"))
        .ok()?;
    let Some(user) = user else {
        tracing::error!(message_id = "kD9pQf4Y", "current user does not exist");
        return None;
    };
    Some(user.name)
}

pub fn client_log_dir() -> Option<camino::Utf8PathBuf> {
    let xdg_state_home = std::env::var("XDG_STATE_HOME").ok().filter(|dir| !dir.is_empty());
    let home = std::env::var("HOME").ok().filter(|dir| !dir.is_empty());
    let state_home = match (xdg_state_home, home) {
        (Some(xdg_state_home), _) => camino::Utf8PathBuf::from(xdg_state_home),
        (None, Some(home)) => camino::Utf8PathBuf::from_iter([home.as_str(), ".local", "state"]),
        (None, None) => return None,
    };
    Some(state_home.join("obscura").join("logs"))
}

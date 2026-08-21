pub mod autostart;
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

pub fn user_config_dir() -> Option<camino::Utf8PathBuf> {
    xdg_base_dir("XDG_CONFIG_HOME", &[".config"])
}

pub fn client_log_dir() -> Option<camino::Utf8PathBuf> {
    Some(xdg_base_dir("XDG_STATE_HOME", &[".local", "state"])?.join("obscura").join("logs"))
}

fn xdg_base_dir(env_var: &str, home_relative: &[&str]) -> Option<camino::Utf8PathBuf> {
    let xdg_dir = std::env::var(env_var).ok().filter(|dir| !dir.is_empty());
    let home = std::env::var("HOME").ok().filter(|dir| !dir.is_empty());
    match (xdg_dir, home) {
        (Some(xdg_dir), _) => Some(camino::Utf8PathBuf::from(xdg_dir)),
        (None, Some(home)) => Some(camino::Utf8PathBuf::from_iter(
            std::iter::once(home.as_str()).chain(home_relative.iter().copied()),
        )),
        (None, None) => None,
    }
}

use crate::{ClientCommand, ClientDebugBundleArgs, ClientLoginArgs, ClientStatusArgs};
use anyhow::Context;
use chrono::{MappedLocalTime, TimeZone};
use obscuravpn_api::types::{AccountId, AccountInfo};
use obscuravpn_client::exit_selection::ExitSelector;
use obscuravpn_client::linux::debug_bundle::create_combined_debug_bundle;
use obscuravpn_client::linux::ipc::{LinuxIpcError, run_command};
use obscuravpn_client::linux::ui_log_dir;
use obscuravpn_client::manager::{Status, TunnelArgs, VpnStatus};
use obscuravpn_client::manager_cmd::{ManagerCmd, ManagerCmdErrorCode};

#[cfg(test)]
mod tests;

#[derive(thiserror::Error, Debug)]
pub enum ClientError {
    #[error("The Obscura API is unreachable.")]
    ApiUnreachable,
    #[error("Insufficient permissions to connect to service.")]
    InsufficientPermissions,
    #[error("Unexpected error. Details: {0:#}")]
    Unexpected(#[from] anyhow::Error),
    #[error("The Obscura VPN service is not running.")]
    NoService,
    #[error("Malformed account ID.")]
    MalformedAccountId,
    #[error("Not logged in.")]
    NotLoggedIn,
    #[error("The CLI ({app_version}) and running Obscura VPN service ({service_version}) are different releases; install matching versions.")]
    VersionMismatch { service_version: String, app_version: String },
    #[error("The connection attempt failed: {0}. The service keeps retrying; run `obscura disconnect` to stop it.")]
    TunnelFailed(String),
    #[error("The service moved off the requested target while waiting and is now {0}. Another client may have changed it.")]
    TargetChanged(String),
}

impl ClientError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NoService => 3,
            Self::InsufficientPermissions => 4,
            Self::VersionMismatch { .. } => 5,
            Self::TunnelFailed(_) => 7,
            Self::TargetChanged(_) => 1,
            Self::ApiUnreachable | Self::Unexpected(_) | Self::MalformedAccountId | Self::NotLoggedIn => 1,
        }
    }
}

impl From<ManagerCmdErrorCode> for ClientError {
    fn from(error: ManagerCmdErrorCode) -> ClientError {
        match error {
            ManagerCmdErrorCode::ApiInvalidAccountId => ClientError::MalformedAccountId,
            ManagerCmdErrorCode::ApiUnreachable => ClientError::ApiUnreachable,
            ManagerCmdErrorCode::NotLoggedIn => ClientError::NotLoggedIn,
            ManagerCmdErrorCode::ApiAssociateAccountConflict
            | ManagerCmdErrorCode::ApiError
            | ManagerCmdErrorCode::ApiNoLongerSupported
            | ManagerCmdErrorCode::ApiRateLimitExceeded
            | ManagerCmdErrorCode::ApiSaleNotFound
            | ManagerCmdErrorCode::ApiSignupLimitExceeded
            | ManagerCmdErrorCode::ConfigSaveError
            | ManagerCmdErrorCode::Other => anyhow::Error::msg(error.as_static_str()).into(),
        }
    }
}

impl From<LinuxIpcError> for ClientError {
    fn from(error: LinuxIpcError) -> ClientError {
        match error {
            LinuxIpcError::InsufficientPermissions => ClientError::InsufficientPermissions,
            LinuxIpcError::NoListener => ClientError::NoService,
            LinuxIpcError::VersionMismatch { service_version, app_version } => ClientError::VersionMismatch { service_version, app_version },
            LinuxIpcError::Other => ClientError::Unexpected(anyhow::Error::msg("unexpected IPC error")),
        }
    }
}

pub async fn run(cmd: ClientCommand) -> Result<(), ClientError> {
    match cmd {
        ClientCommand::AddOperator { users } => crate::add_operator::run_add_operator(users).await,
        ClientCommand::Login(args) => login(args).await,
        ClientCommand::Connect(_args) => go_to_target_state(Some(TunnelArgs { exit: ExitSelector::Any {} })).await,
        ClientCommand::Disconnect(_args) => go_to_target_state(None).await,
        ClientCommand::Status(args) => status(args).await,
        ClientCommand::DebugBundle(args) => debug_bundle(args).await,
    }
}

async fn debug_bundle(args: ClientDebugBundleArgs) -> Result<(), ClientError> {
    let log_dir = ui_log_dir().filter(|dir| dir.is_dir());
    let path = create_combined_debug_bundle(args.feedback, log_dir.as_deref())
        .await
        .map_err(|()| ClientError::Unexpected(anyhow::Error::msg("failed to create debug bundle")))?;
    println!("{path}");
    Ok(())
}

async fn status(args: ClientStatusArgs) -> Result<(), ClientError> {
    let get_account_info_result: Result<AccountInfo, _> = run_command(ManagerCmd::ApiGetAccountInfo {}).await?;
    match get_account_info_result {
        Ok(account_info) => {
            if !args.json {
                println!("Account is {}.", account_info_summary(&account_info))
            }
        }
        Err(error) => eprintln!("Failed to update account info: {}", ClientError::from(error)),
    }
    let mut known_version = None;
    loop {
        let status: Status = run_command(ManagerCmd::GetStatus { known_version }).await??;
        known_version = Some(status.version);
        if args.json {
            let json = serde_json::to_string_pretty(&status)
                .map_err(anyhow::Error::new)
                .context("JSON encoding failed")?;
            println!("{json}");
        } else {
            println!("VPN is {}.", vpn_status_summary(&status.vpn_status));
        }
        if !args.follow {
            break Ok(());
        }
    }
}

async fn login(args: ClientLoginArgs) -> Result<(), ClientError> {
    let account = match args.account {
        Some(account) => account,
        None => {
            if std::io::IsTerminal::is_terminal(&std::io::stdin()) {
                eprint!("Account number: ");
            }
            let mut line = String::new();
            std::io::stdin()
                .read_line(&mut line)
                .map_err(|error| anyhow::Error::new(error).context("failed to read account number from stdin"))?;
            line.trim_end_matches(['\r', '\n']).to_string()
        }
    };
    if account.is_empty() {
        return Err(ClientError::MalformedAccountId);
    }
    let _: () = run_command(ManagerCmd::Login { account_id: AccountId::from_string_unchecked(account), validate: !args.offline }).await??;
    if !args.offline {
        eprintln!("successfully logged in");
    } else {
        eprintln!("set account number in config without checking validity (offline mode)");
    }
    Ok(())
}

async fn go_to_target_state(target_state: Option<TunnelArgs>) -> Result<(), ClientError> {
    run_command::<()>(ManagerCmd::SetTunnelArgs { args: target_state.clone(), active: Some(target_state.is_some()) }).await??;
    eprintln!("updated target state");
    let mut known_version = None;
    let mut target_taken_up = false;
    loop {
        let status: Status = run_command(ManagerCmd::GetStatus { known_version }).await??;
        known_version = Some(status.version);
        eprintln!("{}", vpn_status_summary(&status.vpn_status));
        if target_reached(&status.vpn_status, &target_state)? {
            break;
        }
        target_taken_up = track_target(&status.vpn_status, &target_state, target_taken_up)?;
    }
    eprintln!("reached target state");
    Ok(())
}

fn target_reached(vpn_status: &VpnStatus, target_state: &Option<TunnelArgs>) -> Result<bool, ClientError> {
    match (vpn_status, target_state) {
        (VpnStatus::Connected { exit, .. }, Some(TunnelArgs { exit: exit_selector })) => Ok(exit_selector.matches(exit)),
        (VpnStatus::Disconnected {}, None) => Ok(true),
        (VpnStatus::Connecting { tunnel_args, connect_error: Some(_), .. }, Some(target)) if tunnel_args == target => {
            Err(ClientError::TunnelFailed(vpn_status_summary(vpn_status)))
        }
        _ => Ok(false),
    }
}

/// Reports whether the service has taken up the connect target, and fails once it moves off it, so a target
/// another client replaced cannot keep the CLI waiting forever. The first statuses after a request can still
/// describe the previous target, so only a move after the service took up this one counts. A disconnect is not
/// tracked: its target state has no arguments to compare.
fn track_target(vpn_status: &VpnStatus, target_state: &Option<TunnelArgs>, taken_up: bool) -> Result<bool, ClientError> {
    let Some(target) = target_state else {
        return Ok(taken_up);
    };
    let current = match vpn_status {
        VpnStatus::Connecting { tunnel_args, .. } | VpnStatus::Connected { tunnel_args, .. } => Some(tunnel_args),
        VpnStatus::Disconnected {} => None,
    };
    if current == Some(target) {
        return Ok(true);
    }
    if taken_up {
        return Err(ClientError::TargetChanged(vpn_status_summary(vpn_status)));
    }
    Ok(false)
}

fn vpn_status_summary(vpn_status: &VpnStatus) -> String {
    match vpn_status {
        VpnStatus::Connecting { connect_error: Some(error_code), .. } => {
            format!("connecting (error: \"{}\")", error_code.as_static_str())
        }
        VpnStatus::Connecting { connect_error: None, .. } => "connecting".to_string(),
        VpnStatus::Connected { exit, .. } => format!(
            "connected to {} in {} ({})",
            exit.id,
            exit.city_name,
            exit.city_code.country_code.0.to_uppercase()
        ),
        VpnStatus::Disconnected { .. } => "disconnected".to_string(),
    }
}

fn account_info_summary(account_info: &AccountInfo) -> String {
    let mut summary = String::new();
    if account_info.active {
        if let Some(expiry) = account_info.current_expiry {
            summary += "active";
            if let MappedLocalTime::Single(timestamp) = chrono::Local.timestamp_opt(expiry, 0) {
                summary += &format!(" until {}", timestamp)
            };
        } else {
            summary += "active and subscribed";
        }
    } else {
        summary += "expired (top-up or subscribe to activate)";
    }
    summary
}

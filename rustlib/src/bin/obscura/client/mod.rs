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
use std::io::Write;

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
    let source = status_source(args.local);
    let output = status_output(args.json, args.json_lines);
    let account_summary = match source {
        StatusSource::WithAccountRefresh => {
            let get_account_info_result: Result<AccountInfo, _> = run_command(ManagerCmd::ApiGetAccountInfo {}).await?;
            match get_account_info_result {
                Ok(account_info) => Some(account_info_summary(&account_info)),
                Err(error) => {
                    eprintln!("Failed to update account info: {}", ClientError::from(error));
                    None
                }
            }
        }
        StatusSource::LocalOnly => None,
    };
    write_account_summary_to_stdout(source, account_summary.as_deref(), output)?;
    let mut known_version = None;
    loop {
        let status: Status = run_command(ManagerCmd::GetStatus { known_version }).await??;
        known_version = Some(status.version);
        match write_status_to_stdout(&status, output) {
            Ok(()) => {}
            Err(error) if is_broken_pipe(&error) => {
                // The consumer closed the stream, which ends a followed stream normally.
                break Ok(());
            }
            Err(error) => return Err(error),
        }
        if !args.follow {
            break Ok(());
        }
    }
}

/// Where the reported status information comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusSource {
    /// Ask the service to refresh the account information before reporting the status.
    WithAccountRefresh,
    /// Report only what the service already knows, without asking it to refresh the account.
    LocalOnly,
}

/// Selects the status source from the `--local` flag.
fn status_source(local: bool) -> StatusSource {
    if local {
        StatusSource::LocalOnly
    } else {
        StatusSource::WithAccountRefresh
    }
}

/// The format a status report is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum StatusOutput {
    /// A human-readable summary.
    Text,
    /// One pretty-printed JSON object.
    Json,
    /// One compact JSON object per line, for machine-readable streams.
    JsonLines,
}

/// Selects the output format from the output flags.
fn status_output(json: bool, json_lines: bool) -> StatusOutput {
    if json_lines {
        StatusOutput::JsonLines
    } else if json {
        StatusOutput::Json
    } else {
        StatusOutput::Text
    }
}

/// Writes the account summary line, unless the output is not human-readable or the account was not refreshed.
fn write_account_summary<W: Write>(
    source: StatusSource,
    account_summary: Option<&str>,
    output: StatusOutput,
    out: &mut W,
) -> Result<(), ClientError> {
    if source != StatusSource::WithAccountRefresh || output != StatusOutput::Text {
        return Ok(());
    }
    let Some(account_summary) = account_summary else {
        return Ok(());
    };
    writeln!(out, "Account is {account_summary}.").map_err(anyhow::Error::new)?;
    Ok(())
}

/// Writes the account summary line to stdout.
fn write_account_summary_to_stdout(source: StatusSource, account_summary: Option<&str>, output: StatusOutput) -> Result<(), ClientError> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    write_account_summary(source, account_summary, output, &mut out)
}

/// Writes the status as JSON, as a JSON Lines record, or as a human-readable summary.
fn write_status<W: Write>(status: &Status, output: StatusOutput, out: &mut W) -> Result<(), ClientError> {
    match output {
        StatusOutput::Text => {
            writeln!(out, "VPN is {}.", vpn_status_summary(&status.vpn_status)).map_err(anyhow::Error::new)?;
        }
        StatusOutput::Json => {
            let json = serde_json::to_string_pretty(status)
                .map_err(anyhow::Error::new)
                .context("JSON encoding failed")?;
            writeln!(out, "{json}").map_err(anyhow::Error::new)?;
        }
        StatusOutput::JsonLines => {
            write_json_line(out, status)?;
        }
    }
    Ok(())
}

/// Writes one status record to stdout.
fn write_status_to_stdout(status: &Status, output: StatusOutput) -> Result<(), ClientError> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    write_status(status, output, &mut out)
}

/// Writes one compact JSON record followed by a newline.
fn write_json_line<W: Write, T: serde::Serialize>(writer: &mut W, value: &T) -> anyhow::Result<()> {
    serde_json::to_writer(&mut *writer, value)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

/// Returns true when the error is a broken pipe, which means the consumer closed the output stream.
fn is_broken_pipe(error: &ClientError) -> bool {
    let ClientError::Unexpected(error) = error else {
        return false;
    };
    if let Some(error) = error.downcast_ref::<std::io::Error>() {
        return error.kind() == std::io::ErrorKind::BrokenPipe;
    }
    if let Some(error) = error.downcast_ref::<serde_json::Error>() {
        return error.io_error_kind() == Some(std::io::ErrorKind::BrokenPipe);
    }
    false
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

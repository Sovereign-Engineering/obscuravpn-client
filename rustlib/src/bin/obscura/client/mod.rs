use crate::{
    ClientCommand, ClientDebugBundleArgs, ClientLocationsListArgs, ClientLocationsRefreshArgs, ClientLoginArgs, ClientStatusArgs, LocationsCommand,
    WaitArgs,
};
use anyhow::Context;
use chrono::{MappedLocalTime, TimeZone};
use obscuravpn_api::cmd::ExitList;
use obscuravpn_api::types::{AccountId, AccountInfo, OneExit};
use obscuravpn_client::cached_value::CachedValue;
use obscuravpn_client::exit_selection::ExitSelector;
use obscuravpn_client::linux::debug_bundle::create_combined_debug_bundle;
use obscuravpn_client::linux::ipc::{LinuxIpcError, run_command};
use obscuravpn_client::linux::ui_log_dir;
use obscuravpn_client::manager::{Status, TunnelArgs, VpnStatus};
use obscuravpn_client::manager_cmd::{ManagerCmd, ManagerCmdErrorCode};
use std::future::Future;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

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
    #[error(
        "Timed out after {seconds}s while {step}. The service may still apply the requested state; check the status instead of repeating the command."
    )]
    TimedOut { seconds: u64, step: &'static str },
    #[error("The exit catalogue is not available. Run `obscura locations refresh` to fetch the exits from the API, then run this command again.")]
    CatalogueUnavailable,
    #[error(
        "Timed out after {seconds}s while refreshing the exit catalogue. The catalogue may or may not have been updated; run `obscura locations list` to check."
    )]
    CatalogueRefreshTimedOut { seconds: u64 },
}

impl ClientError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NoService => 3,
            Self::InsufficientPermissions => 4,
            Self::VersionMismatch { .. } => 5,
            Self::TunnelFailed(_) => 7,
            Self::TargetChanged(_) => 1,
            Self::TimedOut { .. } => 6,
            Self::CatalogueUnavailable | Self::CatalogueRefreshTimedOut { .. } => 6,
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
        ClientCommand::Connect(args) => go_to_target_state(args.wait, connect_target()).await,
        ClientCommand::Disconnect(args) => go_to_target_state(args.wait, None).await,
        ClientCommand::Status(args) => status(args).await,
        ClientCommand::Locations(command) => locations(command).await,
        ClientCommand::DebugBundle(args) => debug_bundle(args).await,
    }
}

/// The exit catalogue the service has cached.
type ExitCatalogue = CachedValue<Arc<ExitList>>;

/// The time a catalogue read waits before the catalogue is reported as unavailable.
const CATALOGUE_READ_TIMEOUT_SECONDS: u64 = 5;

/// The time a catalogue refresh waits before it is reported as timed out.
const CATALOGUE_REFRESH_TIMEOUT_SECONDS: u64 = 30;

/// Lists the cached exits, or refreshes the catalogue from the API.
async fn locations(command: LocationsCommand) -> Result<(), ClientError> {
    match command {
        LocationsCommand::List(args) => locations_list(args).await,
        LocationsCommand::Refresh(args) => locations_refresh(args).await,
    }
}

/// Prints the exit catalogue the service has cached.
async fn locations_list(args: ClientLocationsListArgs) -> Result<(), ClientError> {
    let timeout = Timeout::new(args.timeout.unwrap_or(CATALOGUE_READ_TIMEOUT_SECONDS));
    let catalogue = read_catalogue(timeout).await?;
    if args.json {
        write_catalogue_json_to_stdout(&catalogue)
    } else {
        write_catalogue_text_to_stdout(&catalogue)
    }
}

/// Asks the service to fetch the exit catalogue from the API.
async fn locations_refresh(args: ClientLocationsRefreshArgs) -> Result<(), ClientError> {
    let timeout = Timeout::new(args.timeout.unwrap_or(CATALOGUE_REFRESH_TIMEOUT_SECONDS));
    refresh_catalogue(timeout).await?;
    eprintln!("refreshed the exit catalogue");
    Ok(())
}

/// Reads the cached exit catalogue. The service waits for a catalogue to exist, so the read is bounded.
async fn read_catalogue(timeout: Timeout) -> Result<ExitCatalogue, ClientError> {
    read_catalogue_with(timeout, run_command(catalogue_read_command())).await
}

/// Reads the cached exit catalogue under a deadline.
async fn read_catalogue_with<F>(timeout: Timeout, request: F) -> Result<ExitCatalogue, ClientError>
where
    F: Future<Output = Result<Result<ExitCatalogue, ManagerCmdErrorCode>, LinuxIpcError>>,
{
    match tokio::time::timeout_at(timeout.deadline, request).await {
        Ok(result) => Ok(result??),
        Err(_) => Err(ClientError::CatalogueUnavailable),
    }
}

/// Asks the service to fetch the exit catalogue from the API, under a deadline.
async fn refresh_catalogue(timeout: Timeout) -> Result<(), ClientError> {
    refresh_catalogue_with(timeout, run_command::<()>(catalogue_refresh_command())).await
}

/// Asks the service to fetch the exit catalogue from the API, under a deadline.
async fn refresh_catalogue_with<F>(timeout: Timeout, request: F) -> Result<(), ClientError>
where
    F: Future<Output = Result<Result<(), ManagerCmdErrorCode>, LinuxIpcError>>,
{
    match tokio::time::timeout_at(timeout.deadline, request).await {
        Ok(result) => {
            result??;
            Ok(())
        }
        Err(_) => Err(ClientError::CatalogueRefreshTimedOut { seconds: timeout.seconds }),
    }
}

/// The command that reads the cached exit catalogue.
fn catalogue_read_command() -> ManagerCmd {
    ManagerCmd::GetExitList { known_version: None }
}

/// The command that refreshes the exit catalogue from the API.
fn catalogue_refresh_command() -> ManagerCmd {
    ManagerCmd::RefreshExitList { freshness: Duration::ZERO }
}

/// Writes the exit catalogue as a human-readable list, sorted by country, city and exit ID.
fn write_catalogue_text<W: Write>(catalogue: &ExitCatalogue, out: &mut W) -> Result<(), ClientError> {
    let mut exits: Vec<&OneExit> = catalogue.value.exits.iter().collect();
    exits.sort_by(|left, right| {
        let left_key = (&left.city_code.country_code.0, &left.city_code.city_code, &left.id);
        let right_key = (&right.city_code.country_code.0, &right.city_code.city_code, &right.id);
        left_key.cmp(&right_key)
    });
    if exits.is_empty() {
        writeln!(
            out,
            "The exit catalogue is empty. Run `obscura locations refresh` to fetch the exits from the API."
        )
        .map_err(anyhow::Error::new)?;
        return Ok(());
    }
    for exit in exits {
        writeln!(
            out,
            "{}  {}  exit {}  {}  provider {}",
            exit.city_code.country_code.0, exit.city_code.city_code, exit.id, exit.city_name, exit.provider_id
        )
        .map_err(anyhow::Error::new)?;
    }
    Ok(())
}

/// Writes the exit catalogue as JSON, in the shape the service serializes it.
fn write_catalogue_json<W: Write>(catalogue: &ExitCatalogue, out: &mut W) -> Result<(), ClientError> {
    let json = serde_json::to_string_pretty(catalogue)
        .map_err(anyhow::Error::new)
        .context("JSON encoding failed")?;
    writeln!(out, "{json}").map_err(anyhow::Error::new)?;
    Ok(())
}

/// Writes the exit catalogue as a human-readable list to stdout.
fn write_catalogue_text_to_stdout(catalogue: &ExitCatalogue) -> Result<(), ClientError> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    write_catalogue_text(catalogue, &mut out)
}

/// Writes the exit catalogue as JSON to stdout.
fn write_catalogue_json_to_stdout(catalogue: &ExitCatalogue) -> Result<(), ClientError> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    write_catalogue_json(catalogue, &mut out)
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

/// Asks the service to reach the target state, optionally without waiting for it.
async fn go_to_target_state(wait: WaitArgs, target_state: Option<TunnelArgs>) -> Result<(), ClientError> {
    let timeout = wait.timeout.map(Timeout::new);
    let submit = run_with_deadline(timeout, OperationStep::Submit, submit_tunnel_args(&target_state));
    let wait_for_state = if wait.no_wait {
        None
    } else {
        Some(run_with_deadline(
            timeout,
            OperationStep::Wait,
            await_target_state(&target_state, timeout.is_some()),
        ))
    };
    let result = apply_target_state(submit, wait_for_state).await;
    if wait.no_wait && result.is_ok() {
        eprintln!("request accepted; not waiting for the target state");
    }
    result
}

/// The step of a connection operation, used to report where a timeout happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OperationStep {
    /// Sending the requested state to the service.
    Submit,
    /// Waiting for the service to reach the requested state.
    Wait,
}

impl OperationStep {
    /// Describes the step, for the timeout error message.
    fn description(&self) -> &'static str {
        match self {
            Self::Submit => "submitting the target state",
            Self::Wait => "waiting for the target state",
        }
    }
}

/// The deadline of a connection operation.
#[derive(Debug, Clone, Copy)]
struct Timeout {
    /// The requested timeout, in seconds.
    seconds: u64,
    /// The instant the operation has to be finished by.
    deadline: tokio::time::Instant,
}

impl Timeout {
    fn new(seconds: u64) -> Self {
        let deadline = tokio::time::Instant::now()
            .checked_add(Duration::from_secs(seconds))
            .unwrap_or_else(tokio::time::Instant::now);
        Self { seconds, deadline }
    }
}

/// Runs an operation, giving up when the deadline expires.
async fn run_with_deadline<F>(timeout: Option<Timeout>, step: OperationStep, operation: F) -> Result<(), ClientError>
where
    F: Future<Output = Result<(), ClientError>>,
{
    let Some(timeout) = timeout else {
        return operation.await;
    };
    match tokio::time::timeout_at(timeout.deadline, operation).await {
        Ok(result) => result,
        Err(_) => Err(ClientError::TimedOut { seconds: timeout.seconds, step: step.description() }),
    }
}

/// Sends the requested state once and, unless the caller opted out of waiting, waits for the service to reach it.
async fn apply_target_state<S, W>(submit: S, wait: Option<W>) -> Result<(), ClientError>
where
    S: Future<Output = Result<(), ClientError>>,
    W: Future<Output = Result<(), ClientError>>,
{
    submit.await?;
    match wait {
        Some(wait) => wait.await,
        None => Ok(()),
    }
}

/// Sends the requested state to the service. The request is sent exactly once.
async fn submit_tunnel_args(target_state: &Option<TunnelArgs>) -> Result<(), ClientError> {
    run_command::<()>(set_tunnel_args_command(target_state)).await??;
    eprintln!("updated target state");
    Ok(())
}

/// The command that sets the requested tunnel state.
fn set_tunnel_args_command(target_state: &Option<TunnelArgs>) -> ManagerCmd {
    ManagerCmd::SetTunnelArgs { args: target_state.clone(), active: Some(target_state.is_some()) }
}

/// The target state of a connection request: any exit.
fn connect_target() -> Option<TunnelArgs> {
    Some(TunnelArgs { exit: ExitSelector::Any {} })
}

/// Waits until the service reaches the target state.
async fn await_target_state(target_state: &Option<TunnelArgs>, deadline: bool) -> Result<(), ClientError> {
    let mut known_version = None;
    let mut target_taken_up = false;
    loop {
        let status: Status = run_command(ManagerCmd::GetStatus { known_version }).await??;
        known_version = Some(status.version);
        eprintln!("{}", vpn_status_summary(&status.vpn_status));
        if wait_finished(&status.vpn_status, target_state, deadline)? {
            break;
        }
        target_taken_up = track_target(&status.vpn_status, target_state, target_taken_up)?;
    }
    eprintln!("reached target state");
    Ok(())
}

/// Whether the wait is over. Under a deadline a connection error is progress, because the service keeps
/// retrying until the deadline expires. Without a deadline the error ends the wait, so the CLI cannot wait forever.
fn wait_finished(vpn_status: &VpnStatus, target_state: &Option<TunnelArgs>, deadline: bool) -> Result<bool, ClientError> {
    match target_reached(vpn_status, target_state) {
        Err(ClientError::TunnelFailed(_)) if deadline => Ok(false),
        result => result,
    }
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

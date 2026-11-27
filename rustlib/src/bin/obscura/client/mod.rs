use crate::{
    AutoConnectCommand, ClientCommand, ClientConnectArgs, ClientDebugBundleArgs, ClientDnsBlockSetArgs, ClientLocationsListArgs,
    ClientLocationsRefreshArgs, ClientLoginArgs, ClientStatusArgs, DnsBlockCommand, DnsCommand, DnsMode, LanCommand, LocationsCommand, WaitArgs,
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
use obscuravpn_client::network_config::{DnsContentBlock, DnsContentBlockConfig};
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
    #[error("No exit in the catalogue matches {requested}. Run `obscura locations list` to see the available exits.")]
    UnknownSelection { requested: String },
    #[error("Timed out after {seconds}s while looking up the exit in the catalogue. Run `obscura locations list` to see the available exits.")]
    SelectionLookupTimedOut { seconds: u64 },
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
            Self::SelectionLookupTimedOut { .. } => 6,
            Self::UnknownSelection { .. } => 1,
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
        ClientCommand::Connect(args) => connect(args).await,
        ClientCommand::Disconnect(args) => go_to_target_state(args.wait, None).await,
        ClientCommand::Status(args) => status(args).await,
        ClientCommand::Locations(command) => locations(command).await,
        ClientCommand::AutoConnect(command) => auto_connect(command).await,
        ClientCommand::Lan(command) => lan(command).await,
        ClientCommand::Dns(command) => dns(command).await,
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
    read_catalogue_with(timeout, ClientError::CatalogueUnavailable, run_command(catalogue_read_command())).await
}

/// Reads the cached exit catalogue under a deadline, reporting the given error when the deadline expires.
async fn read_catalogue_with<F>(timeout: Timeout, timeout_error: ClientError, request: F) -> Result<ExitCatalogue, ClientError>
where
    F: Future<Output = Result<Result<ExitCatalogue, ManagerCmdErrorCode>, LinuxIpcError>>,
{
    match tokio::time::timeout_at(timeout.deadline, request).await {
        Ok(result) => Ok(result??),
        Err(_) => Err(timeout_error),
    }
}

/// Reads the catalogue for a connect request. The deadline of the operation bounds the lookup; without
/// one the catalogue's own default applies.
async fn read_catalogue_for_selection(timeout: Option<Timeout>) -> Result<ExitCatalogue, ClientError> {
    let (deadline, timeout_error) = selection_lookup(timeout);
    read_catalogue_with(deadline, timeout_error, run_command(catalogue_read_command())).await
}

/// The deadline and the error of the catalogue lookup a connect request makes.
fn selection_lookup(timeout: Option<Timeout>) -> (Timeout, ClientError) {
    match timeout {
        Some(timeout) => (timeout, ClientError::SelectionLookupTimedOut { seconds: timeout.seconds }),
        None => (Timeout::new(CATALOGUE_READ_TIMEOUT_SECONDS), ClientError::CatalogueUnavailable),
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

/// A setting of the service that the CLI can read and change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServiceSetting {
    /// Whether the service connects automatically.
    AutoConnect,
    /// Whether the tunnel may reach the local network.
    LocalNetworkAccess,
}

impl ServiceSetting {
    /// The name of the setting, as it is reported.
    fn label(&self) -> &'static str {
        match self {
            Self::AutoConnect => "Auto-connect",
            Self::LocalNetworkAccess => "Local network access",
        }
    }

    /// The state of the setting, as it is reported.
    fn state(&self, value: bool) -> &'static str {
        match (self, value) {
            (Self::AutoConnect, true) => "on",
            (Self::AutoConnect, false) => "off",
            (Self::LocalNetworkAccess, true) => "allowed",
            (Self::LocalNetworkAccess, false) => "blocked",
        }
    }

    /// The value of the setting, as the service reports it.
    fn value(&self, status: &Status) -> bool {
        match self {
            Self::AutoConnect => status.auto_connect,
            Self::LocalNetworkAccess => status.local_network_access,
        }
    }

    /// The JSON projection of the setting.
    fn json(&self, value: bool) -> serde_json::Value {
        match self {
            Self::AutoConnect => serde_json::json!({ "autoConnect": value }),
            Self::LocalNetworkAccess => serde_json::json!({ "localNetworkAccess": value }),
        }
    }

    /// The command that changes the setting.
    fn command(&self, value: bool) -> ManagerCmd {
        match self {
            Self::AutoConnect => ManagerCmd::SetAutoConnect { enable: value },
            Self::LocalNetworkAccess => ManagerCmd::SetLocalNetworkAccess { enable: value },
        }
    }
}

/// Reports or changes the automatic connection.
async fn auto_connect(command: AutoConnectCommand) -> Result<(), ClientError> {
    match command {
        AutoConnectCommand::Get(args) => setting_get(ServiceSetting::AutoConnect, args.json).await,
        AutoConnectCommand::Set(args) => setting_set(ServiceSetting::AutoConnect, args.value.enabled()).await,
    }
}

/// Reports or changes the access to the local network.
async fn lan(command: LanCommand) -> Result<(), ClientError> {
    match command {
        LanCommand::Get(args) => setting_get(ServiceSetting::LocalNetworkAccess, args.json).await,
        LanCommand::Set(args) => setting_set(ServiceSetting::LocalNetworkAccess, args.value.enabled()).await,
    }
}

/// The command that reads the status the settings are reported in.
fn status_command() -> ManagerCmd {
    ManagerCmd::GetStatus { known_version: None }
}

/// Reads a setting. The only request is the status read, so a read never changes a setting.
async fn setting_get_with<F, R>(setting: ServiceSetting, request: F) -> Result<bool, ClientError>
where
    F: FnOnce(ManagerCmd) -> R,
    R: Future<Output = Result<Result<Status, ManagerCmdErrorCode>, LinuxIpcError>>,
{
    let status = request(status_command()).await??;
    Ok(setting.value(&status))
}

/// Changes a setting. The only request is the setting command, so a change never touches the tunnel.
async fn setting_set_with<F, R>(setting: ServiceSetting, value: bool, request: F) -> Result<(), ClientError>
where
    F: FnOnce(ManagerCmd) -> R,
    R: Future<Output = Result<Result<(), ManagerCmdErrorCode>, LinuxIpcError>>,
{
    request(setting.command(value)).await??;
    Ok(())
}

/// Reads a setting from the service and reports it.
async fn setting_get(setting: ServiceSetting, json: bool) -> Result<(), ClientError> {
    let value = setting_get_with(setting, run_command).await?;
    write_setting_to_stdout(setting, json, value)
}

/// Changes a setting in the service and reports the new state.
async fn setting_set(setting: ServiceSetting, value: bool) -> Result<(), ClientError> {
    setting_set_with(setting, value, run_command).await?;
    write_setting_to_stdout(setting, false, value)
}

/// Writes a setting as JSON, or as a human-readable line.
fn write_setting<W: Write>(setting: ServiceSetting, json: bool, value: bool, out: &mut W) -> Result<(), ClientError> {
    if json {
        let json = serde_json::to_string(&setting.json(value))
            .map_err(anyhow::Error::new)
            .context("JSON encoding failed")?;
        writeln!(out, "{json}").map_err(anyhow::Error::new)?;
    } else {
        writeln!(out, "{} is {}.", setting.label(), setting.state(value)).map_err(anyhow::Error::new)?;
    }
    Ok(())
}

/// Writes a setting to stdout.
fn write_setting_to_stdout(setting: ServiceSetting, json: bool, value: bool) -> Result<(), ClientError> {
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    write_setting(setting, json, value, &mut out)
}

/// The DNS settings of the service, as `obscura dns get` reports them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DnsSettings {
    use_system_dns: bool,
    #[serde(rename = "dnsContentBlock")]
    block: DnsContentBlock,
}

/// Reports or changes the DNS mode and the content blockers.
async fn dns(command: DnsCommand) -> Result<(), ClientError> {
    let stdout = std::io::stdout();
    match command {
        DnsCommand::Get(args) => {
            let settings = dns_get_with(run_command).await?;
            write_dns(&settings, args.json, &mut stdout.lock())
        }
        DnsCommand::Mode(args) => {
            dns_set_mode_with(args.value, run_command).await?;
            write_dns_mode(args.value.uses_system_dns(), &mut stdout.lock())
        }
        DnsCommand::Block(DnsBlockCommand::Set(args)) => {
            let config = content_block(&args);
            dns_set_block_with(config, run_command).await?;
            write_content_block(config, &mut stdout.lock())
        }
    }
}

/// The command that changes the DNS mode.
fn dns_mode_command(mode: DnsMode) -> ManagerCmd {
    ManagerCmd::SetUseSystemDns { enable: mode.uses_system_dns() }
}

/// The command that replaces every content blocker.
fn content_block_command(config: DnsContentBlockConfig) -> ManagerCmd {
    ManagerCmd::SetDnsContentBlock { value: DnsContentBlock::from_config(config) }
}

/// The content blockers the arguments name.
fn content_block(args: &ClientDnsBlockSetArgs) -> DnsContentBlockConfig {
    DnsContentBlockConfig {
        ad: args.ads.enabled(),
        tracker: args.trackers.enabled(),
        malware: args.malware.enabled(),
        adult: args.adult.enabled(),
        gambling: args.gambling.enabled(),
        social_media: args.social_media.enabled(),
    }
}

/// Reads the DNS settings. The only request is the status read, so a read never changes a setting.
async fn dns_get_with<F, R>(request: F) -> Result<DnsSettings, ClientError>
where
    F: FnOnce(ManagerCmd) -> R,
    R: Future<Output = Result<Result<Status, ManagerCmdErrorCode>, LinuxIpcError>>,
{
    let status = request(status_command()).await??;
    Ok(DnsSettings { use_system_dns: status.use_system_dns, block: status.dns_content_block })
}

/// Changes the DNS mode. The only request is the mode command, so the content blockers stay as they are.
async fn dns_set_mode_with<F, R>(mode: DnsMode, request: F) -> Result<(), ClientError>
where
    F: FnOnce(ManagerCmd) -> R,
    R: Future<Output = Result<Result<(), ManagerCmdErrorCode>, LinuxIpcError>>,
{
    request(dns_mode_command(mode)).await??;
    Ok(())
}

/// Replaces the content blockers. The only request is the blocker command, so the DNS mode stays as it is.
async fn dns_set_block_with<F, R>(config: DnsContentBlockConfig, request: F) -> Result<(), ClientError>
where
    F: FnOnce(ManagerCmd) -> R,
    R: Future<Output = Result<Result<(), ManagerCmdErrorCode>, LinuxIpcError>>,
{
    request(content_block_command(config)).await??;
    Ok(())
}

/// Writes the DNS settings as JSON, or as human-readable lines.
fn write_dns<W: Write>(settings: &DnsSettings, json: bool, out: &mut W) -> Result<(), ClientError> {
    if json {
        let json = serde_json::to_string(settings)
            .map_err(anyhow::Error::new)
            .context("JSON encoding failed")?;
        writeln!(out, "{json}").map_err(anyhow::Error::new)?;
        return Ok(());
    }
    write_dns_mode(settings.use_system_dns, out)?;
    write_content_block(settings.block.config(), out)
}

/// Writes the DNS mode as a human-readable line.
fn write_dns_mode<W: Write>(use_system_dns: bool, out: &mut W) -> Result<(), ClientError> {
    let mode = if use_system_dns { "system" } else { "default" };
    writeln!(out, "DNS mode is {mode}.").map_err(anyhow::Error::new)?;
    Ok(())
}

/// Writes every content blocker as a human-readable line.
fn write_content_block<W: Write>(config: DnsContentBlockConfig, out: &mut W) -> Result<(), ClientError> {
    for (label, enabled) in [
        ("Ads", config.ad),
        ("Trackers", config.tracker),
        ("Malware", config.malware),
        ("Adult content", config.adult),
        ("Gambling", config.gambling),
        ("Social media", config.social_media),
    ] {
        let state = if enabled { "on" } else { "off" };
        writeln!(out, "{label} blocking is {state}.").map_err(anyhow::Error::new)?;
    }
    Ok(())
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
    go_to_target_state_with(wait, target_state, timeout, submit_tunnel_args).await
}

/// Asks the service to reach the target state under a deadline that also covers the steps before it.
async fn go_to_target_state_with<S, M>(
    wait: WaitArgs,
    target_state: Option<TunnelArgs>,
    timeout: Option<Timeout>,
    submit: S,
) -> Result<(), ClientError>
where
    S: FnOnce(Option<TunnelArgs>) -> M,
    M: Future<Output = Result<(), ClientError>>,
{
    let submit = run_with_deadline(timeout, OperationStep::Submit, submit(target_state.clone()));
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

/// Connects the tunnel, to any exit or to the exit the user chose.
async fn connect(args: ClientConnectArgs) -> Result<(), ClientError> {
    let timeout = args.wait.timeout.map(Timeout::new);
    let requested = requested_exit(
        args.selection.country.as_deref(),
        args.selection.city.as_deref(),
        args.selection.exit.as_deref(),
    );
    connect_with(&requested, args.wait, timeout, read_catalogue_for_selection, submit_tunnel_args).await
}

/// Connects the tunnel to the requested exit. The catalogue is read before the tunnel is changed, so a
/// selection that matches no exit leaves the tunnel alone.
async fn connect_with<R, F, S, M>(requested: &RequestedExit, wait: WaitArgs, timeout: Option<Timeout>, read: R, submit: S) -> Result<(), ClientError>
where
    R: FnOnce(Option<Timeout>) -> F,
    F: Future<Output = Result<ExitCatalogue, ClientError>>,
    S: FnOnce(Option<TunnelArgs>) -> M,
    M: Future<Output = Result<(), ClientError>>,
{
    let target = resolve_target(requested, timeout, read).await?;
    go_to_target_state_with(wait, target, timeout, submit).await
}

/// Resolves the requested exit into the target state of the connection.
async fn resolve_target<R, F>(requested: &RequestedExit, timeout: Option<Timeout>, read: R) -> Result<Option<TunnelArgs>, ClientError>
where
    R: FnOnce(Option<Timeout>) -> F,
    F: Future<Output = Result<ExitCatalogue, ClientError>>,
{
    match requested {
        RequestedExit::Any => Ok(connect_target()),
        requested => {
            let catalogue = read(timeout).await?;
            let selector = resolve_selection(&catalogue, requested)?;
            Ok(Some(TunnelArgs { exit: selector }))
        }
    }
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
async fn submit_tunnel_args(target_state: Option<TunnelArgs>) -> Result<(), ClientError> {
    run_command::<()>(set_tunnel_args_command(&target_state)).await??;
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

/// The exit the user asked to connect to.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RequestedExit {
    /// Any exit.
    Any,
    /// An exit in a country.
    Country { country_code: String },
    /// An exit in a city.
    City { country_code: String, city_code: String },
    /// One specific exit.
    Exit { id: String },
}

/// The exit the user asked for, from the selection flags.
fn requested_exit(country: Option<&str>, city: Option<&str>, exit: Option<&str>) -> RequestedExit {
    if let Some(exit) = exit {
        return RequestedExit::Exit { id: exit.to_string() };
    }
    match (country, city) {
        (Some(country), Some(city)) => RequestedExit::City { country_code: country.to_string(), city_code: city.to_string() },
        (Some(country), None) => RequestedExit::Country { country_code: country.to_string() },
        // A city without a country is rejected by the argument parser.
        _ => RequestedExit::Any,
    }
}

/// Resolves the requested exit against the catalogue, using the canonical values of the matched exit.
fn resolve_selection(catalogue: &ExitCatalogue, requested: &RequestedExit) -> Result<ExitSelector, ClientError> {
    match requested {
        RequestedExit::Any => Ok(ExitSelector::Any {}),
        RequestedExit::Country { country_code } => {
            let matched = catalogue
                .value
                .exits
                .iter()
                .find(|exit| equals_ignoring_case(&exit.city_code.country_code.0, country_code))
                .ok_or_else(|| unknown_selection(requested))?;
            Ok(ExitSelector::Country { country_code: matched.city_code.country_code.clone() })
        }
        RequestedExit::City { country_code, city_code } => {
            let matched = catalogue
                .value
                .exits
                .iter()
                .find(|exit| {
                    equals_ignoring_case(&exit.city_code.country_code.0, country_code) && equals_ignoring_case(&exit.city_code.city_code, city_code)
                })
                .ok_or_else(|| unknown_selection(requested))?;
            Ok(ExitSelector::City { city_code: matched.city_code.clone() })
        }
        RequestedExit::Exit { id } => {
            let matched = catalogue
                .value
                .exits
                .iter()
                .find(|exit| exit.id == *id)
                .ok_or_else(|| unknown_selection(requested))?;
            Ok(ExitSelector::Exit { id: matched.id.clone() })
        }
    }
}

/// Compares two strings, ignoring their case.
fn equals_ignoring_case(left: &str, right: &str) -> bool {
    left.to_lowercase() == right.to_lowercase()
}

/// The error for a selection that no exit in the catalogue matches.
fn unknown_selection(requested: &RequestedExit) -> ClientError {
    ClientError::UnknownSelection { requested: describe_selection(requested) }
}

/// Describes the requested exit, for an error message.
fn describe_selection(requested: &RequestedExit) -> String {
    match requested {
        RequestedExit::Any => "any exit".to_string(),
        RequestedExit::Country { country_code } => format!("country \"{country_code}\""),
        RequestedExit::City { country_code, city_code } => format!("city \"{city_code}\" in country \"{country_code}\""),
        RequestedExit::Exit { id } => format!("exit \"{id}\""),
    }
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

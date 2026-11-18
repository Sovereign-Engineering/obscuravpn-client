use super::*;
use crate::{
    Cli, ClientCommand, ClientConnectArgs, ClientDisconnectArgs, ClientLocationsListArgs, ClientLocationsRefreshArgs, ClientStatusArgs, Command,
    LocationsCommand,
};
use clap::Parser;
use obscuravpn_api::cmd::ExitList;
use obscuravpn_api::types::{CityCode, CountryCode, OneExit, OneRelay, WgPubkey};
use obscuravpn_client::cached_value::CachedValue;
use obscuravpn_client::config::feature_flags::FeatureFlags;
use obscuravpn_client::errors::ConnectErrorCode;
use obscuravpn_client::network_config::DnsContentBlock;
use obscuravpn_client::quicwg::TransportKind;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::SystemTime;
use uuid::Uuid;

fn status_args(args: &[&str]) -> ClientStatusArgs {
    let cli = Cli::try_parse_from(args).expect("failed to parse arguments");
    let Command::Client(ClientCommand::Status(status_args)) = cli.command else {
        panic!("expected a status command, got {:?}", cli.command);
    };
    status_args
}

fn connect_args(args: &[&str]) -> ClientConnectArgs {
    let cli = Cli::try_parse_from(args).expect("failed to parse arguments");
    let Command::Client(ClientCommand::Connect(connect_args)) = cli.command else {
        panic!("expected a connect command, got {:?}", cli.command);
    };
    connect_args
}

fn disconnect_args(args: &[&str]) -> ClientDisconnectArgs {
    let cli = Cli::try_parse_from(args).expect("failed to parse arguments");
    let Command::Client(ClientCommand::Disconnect(disconnect_args)) = cli.command else {
        panic!("expected a disconnect command, got {:?}", cli.command);
    };
    disconnect_args
}

fn locations_list_args(args: &[&str]) -> ClientLocationsListArgs {
    let cli = Cli::try_parse_from(args).expect("failed to parse arguments");
    let Command::Client(ClientCommand::Locations(LocationsCommand::List(list_args))) = cli.command else {
        panic!("expected a locations list command, got {:?}", cli.command);
    };
    list_args
}

fn locations_refresh_args(args: &[&str]) -> ClientLocationsRefreshArgs {
    let cli = Cli::try_parse_from(args).expect("failed to parse arguments");
    let Command::Client(ClientCommand::Locations(LocationsCommand::Refresh(refresh_args))) = cli.command else {
        panic!("expected a locations refresh command, got {:?}", cli.command);
    };
    refresh_args
}

/// The exit catalogue the service reports.
fn synthetic_catalogue(exits: Vec<OneExit>) -> ExitCatalogue {
    CachedValue {
        version: b"catalogue-version".to_vec(),
        last_updated: SystemTime::UNIX_EPOCH,
        value: Arc::new(ExitList { exits }),
    }
}

/// An exit with the identifiers the catalogue reports: an exit ID and a provider ID.
fn synthetic_catalogue_exit(id: &str, country_code: &str, city_code: &str, city_name: &str, provider_id: &str) -> OneExit {
    OneExit {
        id: id.to_string(),
        city_code: CityCode { country_code: CountryCode(country_code.to_string()), city_code: city_code.to_string() },
        city_name: city_name.to_string(),
        provider_id: provider_id.to_string(),
        provider_url: "https://provider.invalid".to_string(),
        provider_name: "Provider".to_string(),
        provider_homepage_url: "https://provider.invalid".to_string(),
        datacenter_id: 1,
        tier: 1,
    }
}

/// The exit a synthetic connected status reports.
fn synthetic_exit(id: &str, country_code: &str, city_code: &str) -> OneExit {
    OneExit {
        id: id.to_string(),
        city_code: CityCode { country_code: CountryCode(country_code.to_string()), city_code: city_code.to_string() },
        city_name: city_code.to_string(),
        provider_id: format!("{country_code}-{city_code}-wg-301"),
        provider_url: "https://provider.invalid".to_string(),
        provider_name: "Provider".to_string(),
        provider_homepage_url: "https://provider.invalid".to_string(),
        datacenter_id: 1,
        tier: 1,
    }
}

/// The relay a synthetic connected status reports.
fn synthetic_relay(country_code: &str, city_code: &str) -> OneRelay {
    OneRelay {
        id: format!("{city_code}-relay"),
        city_code: CityCode { country_code: CountryCode(country_code.to_string()), city_code: city_code.to_string() },
        city_name: city_code.to_string(),
        preferred_exits: Vec::new(),
        ip_v4: std::net::Ipv4Addr::new(192, 0, 2, 1),
        ports: vec![443],
        tls_cert: Vec::new(),
    }
}

/// A status that reports a connected tunnel at the given exit.
fn connected_status(exit: OneExit) -> VpnStatus {
    VpnStatus::Connected {
        tunnel_args: TunnelArgs { exit: ExitSelector::Any {} },
        exit,
        relay: synthetic_relay("US", "nyc"),
        client_public_key: WgPubkey([0u8; 32]),
        exit_public_key: WgPubkey([1u8; 32]),
        transport: TransportKind::Quic,
    }
}

fn synthetic_status() -> Status {
    Status {
        version: Uuid::new_v4(),
        vpn_status: VpnStatus::Disconnected {},
        account_id: None,
        in_new_account_flow: false,
        pinned_locations: Vec::new(),
        last_chosen_exit: ExitSelector::default(),
        last_exit: ExitSelector::default(),
        api_url: "https://api.obscura.invalid".to_string(),
        account: None,
        auto_connect: false,
        feature_flags: FeatureFlags::default(),
        feature_flag_keys: Vec::new(),
        use_system_dns: true,
        local_network_access: false,
        tailscale_bypass: false,
        wireguard_bypass: false,
        dns_content_block: DnsContentBlock::default(),
    }
}

#[test]
fn cli_ipc_exit_codes() {
    for (error, expected) in [
        (LinuxIpcError::NoListener, 3),
        (LinuxIpcError::InsufficientPermissions, 4),
        (
            LinuxIpcError::VersionMismatch { service_version: "service-release".into(), app_version: "cli-release".into() },
            5,
        ),
        (LinuxIpcError::Other, 1),
    ] {
        let error = ClientError::from(error);
        assert_eq!(error.exit_code(), expected);
        if expected == 5 {
            assert!(error.to_string().contains("service-release"));
            assert!(error.to_string().contains("cli-release"));
        }
    }
}

#[test]
fn cli_service_errors_keep_generic_exit_code() {
    for error in [
        ManagerCmdErrorCode::ApiAssociateAccountConflict,
        ManagerCmdErrorCode::ApiError,
        ManagerCmdErrorCode::ApiInvalidAccountId,
        ManagerCmdErrorCode::ApiNoLongerSupported,
        ManagerCmdErrorCode::ApiRateLimitExceeded,
        ManagerCmdErrorCode::ApiSaleNotFound,
        ManagerCmdErrorCode::ApiSignupLimitExceeded,
        ManagerCmdErrorCode::ApiUnreachable,
        ManagerCmdErrorCode::ConfigSaveError,
        ManagerCmdErrorCode::NotLoggedIn,
        ManagerCmdErrorCode::Other,
    ] {
        assert_eq!(ClientError::from(error).exit_code(), 1);
    }
    assert_eq!(ClientError::from(anyhow::anyhow!("unknown failure")).exit_code(), 1);
    let error = crate::Cli::try_parse_from(["obscura", "unknown-command"]).err().unwrap();
    assert_eq!(error.exit_code(), 2);
}

#[test]
fn cli_connection_error_ends_connect_wait() {
    let target = Some(TunnelArgs::default());
    let status = VpnStatus::Connecting {
        tunnel_args: TunnelArgs::default(),
        connect_error: Some(ConnectErrorCode::NotLoggedIn),
        reconnecting: false,
    };
    let error = target_reached(&status, &target).unwrap_err();
    assert_eq!(error.exit_code(), 7);
    assert!(error.to_string().contains(&vpn_status_summary(&status)));
    // A previous connection failure must not prevent waiting for disconnect.
    assert!(!target_reached(&status, &None).unwrap());
    let other_target = Some(TunnelArgs { exit: ExitSelector::Exit { id: "different-exit".into() } });
    assert!(!target_reached(&status, &other_target).unwrap());
}

#[test]
fn cli_connect_wait_ends_when_the_target_changes() {
    let target = Some(TunnelArgs::default());
    let ours = VpnStatus::Connecting { tunnel_args: TunnelArgs::default(), connect_error: None, reconnecting: false };
    let other = VpnStatus::Connecting {
        tunnel_args: TunnelArgs { exit: ExitSelector::Exit { id: "different-exit".into() } },
        connect_error: None,
        reconnecting: false,
    };
    // Before the service takes up the target, a previous target or state is not a change.
    assert!(!track_target(&other, &target, false).unwrap());
    assert!(!track_target(&VpnStatus::Disconnected {}, &target, false).unwrap());
    assert!(track_target(&ours, &target, false).unwrap());
    assert!(track_target(&ours, &target, true).unwrap());
    // After it, another target or a disconnect ends the wait with the generic code.
    let error = track_target(&other, &target, true).unwrap_err();
    assert_eq!(error.exit_code(), 1);
    assert!(error.to_string().contains(&vpn_status_summary(&other)));
    assert_eq!(track_target(&VpnStatus::Disconnected {}, &target, true).unwrap_err().exit_code(), 1);
    // A disconnect is not tracked.
    assert!(track_target(&other, &None, true).is_ok());
}

#[test]
fn cli_timeout_keeps_waiting_through_connection_errors() {
    let target = Some(TunnelArgs::default());
    let failing = VpnStatus::Connecting {
        tunnel_args: TunnelArgs::default(),
        connect_error: Some(ConnectErrorCode::NoInternet),
        reconnecting: false,
    };
    // The service keeps retrying, so under a deadline the error is progress, not the end of the wait.
    assert!(!wait_finished(&failing, &target, true).unwrap());
    assert_eq!(wait_finished(&failing, &target, false).unwrap_err().exit_code(), 7);
    // A deadline does not change how a reached target is detected.
    assert!(wait_finished(&VpnStatus::Disconnected {}, &None, true).unwrap());
}

#[test]
fn cli_wait_preserves_pending_and_disconnected_states() {
    let target = Some(TunnelArgs::default());
    let connecting = VpnStatus::Connecting { tunnel_args: TunnelArgs::default(), connect_error: None, reconnecting: false };
    assert!(!target_reached(&connecting, &target).unwrap());
    assert!(!target_reached(&VpnStatus::Disconnected {}, &target).unwrap());
    assert!(target_reached(&VpnStatus::Disconnected {}, &None).unwrap());
}

/// A writer that fails every write, to check that output errors are reported.
struct FailingWriter;

impl std::io::Write for FailingWriter {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("write failed"))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::other("flush failed"))
    }
}

#[test]
fn cli_status_local_flag() {
    let args = status_args(&["obscura", "status", "--local"]);
    assert!(args.local);
    assert_eq!(status_source(args.local), StatusSource::LocalOnly);

    let args = status_args(&["obscura", "status", "--local", "--json"]);
    assert!(args.local);
    assert!(args.json);
    assert_eq!(status_source(args.local), StatusSource::LocalOnly);

    let args = status_args(&["obscura", "status"]);
    assert!(!args.local);
    assert_eq!(status_source(args.local), StatusSource::WithAccountRefresh);

    let args = status_args(&["obscura", "status", "--json", "--follow"]);
    assert!(!args.local);
    assert!(args.json);
    assert!(args.follow);
}

#[test]
fn cli_local_status_keeps_json_schema() {
    let status = synthetic_status();

    let mut local = Vec::new();
    write_account_summary(StatusSource::LocalOnly, Some("active"), StatusOutput::Json, &mut local).unwrap();
    write_status(&status, StatusOutput::Json, &mut local).unwrap();
    let mut default = Vec::new();
    write_account_summary(StatusSource::WithAccountRefresh, Some("active"), StatusOutput::Json, &mut default).unwrap();
    write_status(&status, StatusOutput::Json, &mut default).unwrap();
    assert_eq!(local, default, "the JSON output must not depend on the account refresh");
    assert!(!String::from_utf8(local).unwrap().contains("Account is"));

    let mut local_text = Vec::new();
    write_account_summary(StatusSource::LocalOnly, Some("active"), StatusOutput::Text, &mut local_text).unwrap();
    write_status(&status, StatusOutput::Text, &mut local_text).unwrap();
    let local_text = String::from_utf8(local_text).unwrap();
    assert!(!local_text.contains("Account is"), "the account line must be absent in local mode");
    assert!(local_text.contains("VPN is "));

    let mut default_text = Vec::new();
    write_account_summary(StatusSource::WithAccountRefresh, Some("active"), StatusOutput::Text, &mut default_text).unwrap();
    write_status(&status, StatusOutput::Text, &mut default_text).unwrap();
    assert!(String::from_utf8(default_text).unwrap().contains("Account is active."));
}

#[test]
fn cli_status_flags() {
    let args = status_args(&["obscura", "status"]);
    assert!(!args.json);
    assert!(!args.json_lines);
    assert!(!args.follow);
    assert_eq!(status_output(args.json, args.json_lines), StatusOutput::Text);

    let args = status_args(&["obscura", "status", "--json"]);
    assert!(args.json);
    assert!(!args.json_lines);
    assert_eq!(status_output(args.json, args.json_lines), StatusOutput::Json);

    let args = status_args(&["obscura", "status", "--json", "--follow"]);
    assert!(args.json);
    assert!(!args.json_lines);
    assert!(args.follow);

    let args = status_args(&["obscura", "status", "--json-lines"]);
    assert!(!args.json);
    assert!(args.json_lines);
    assert_eq!(status_output(args.json, args.json_lines), StatusOutput::JsonLines);

    let args = status_args(&["obscura", "status", "--json-lines", "--follow"]);
    assert!(args.json_lines);
    assert!(args.follow);
    assert_eq!(status_output(args.json, args.json_lines), StatusOutput::JsonLines);

    assert!(
        Cli::try_parse_from(["obscura", "status", "--json", "--json-lines"]).is_err(),
        "--json and --json-lines must not be usable together"
    );
}

#[test]
fn cli_json_lines_are_records() {
    let mut status = synthetic_status();
    status.api_url = "https://api.obscura.invalid\nsecond line".to_string();
    let other = synthetic_status();

    let mut out = Vec::new();
    write_status(&status, StatusOutput::JsonLines, &mut out).unwrap();
    write_status(&other, StatusOutput::JsonLines, &mut out).unwrap();

    let text = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "each record must be exactly one physical line");
    for line in &lines {
        serde_json::from_str::<serde_json::Value>(line).expect("each line must be a JSON value");
    }
    assert!(lines[0].contains("\\n"), "a newline inside a string must stay escaped");
    assert!(text.ends_with('\n'), "every record must end with a newline");

    let mut summary = Vec::new();
    write_account_summary(StatusSource::WithAccountRefresh, Some("active"), StatusOutput::JsonLines, &mut summary).unwrap();
    assert!(summary.is_empty(), "a JSON Lines stream must not contain the account line");

    let mut failing = FailingWriter;
    let error = write_status(&status, StatusOutput::JsonLines, &mut failing).unwrap_err();
    assert!(!is_broken_pipe(&error), "a failing writer is not a broken pipe");

    let broken = ClientError::Unexpected(anyhow::Error::new(std::io::Error::from(std::io::ErrorKind::BrokenPipe)));
    assert!(is_broken_pipe(&broken), "a closed pipe ends the stream");
    let broken_json = ClientError::Unexpected(anyhow::Error::new(serde_json::Error::io(std::io::Error::from(
        std::io::ErrorKind::BrokenPipe,
    ))));
    assert!(is_broken_pipe(&broken_json), "a closed pipe during encoding ends the stream");
}

#[test]
fn cli_connect_wait_flags() {
    let args = connect_args(&["obscura", "connect"]);
    assert!(!args.wait.no_wait);
    assert_eq!(args.wait.timeout, None);

    let args = connect_args(&["obscura", "connect", "--no-wait"]);
    assert!(args.wait.no_wait);
    assert_eq!(args.wait.timeout, None);

    let args = connect_args(&["obscura", "connect", "--timeout", "10"]);
    assert!(!args.wait.no_wait);
    assert_eq!(args.wait.timeout, Some(10));

    let args = disconnect_args(&["obscura", "disconnect"]);
    assert!(!args.wait.no_wait);
    assert_eq!(args.wait.timeout, None);

    let args = disconnect_args(&["obscura", "disconnect", "--no-wait"]);
    assert!(args.wait.no_wait);
    assert_eq!(args.wait.timeout, None);

    let args = disconnect_args(&["obscura", "disconnect", "--timeout", "30"]);
    assert!(!args.wait.no_wait);
    assert_eq!(args.wait.timeout, Some(30));

    assert!(
        Cli::try_parse_from(["obscura", "connect", "--no-wait", "--timeout", "10"]).is_err(),
        "--no-wait and --timeout must not be usable together"
    );
    assert!(
        Cli::try_parse_from(["obscura", "disconnect", "--no-wait", "--timeout", "10"]).is_err(),
        "--no-wait and --timeout must not be usable together"
    );
    for invalid in ["0", "3601", "abc", "-1"] {
        assert!(
            Cli::try_parse_from(["obscura", "connect", "--timeout", invalid]).is_err(),
            "--timeout {invalid} must be rejected"
        );
    }
    assert!(Cli::try_parse_from(["obscura", "connect", "--timeout", "3600"]).is_ok());
}

#[test]
fn cli_target_match_requires_matching_exit() {
    let status = connected_status(synthetic_exit("NYC-001", "US", "nyc"));

    let matching = Some(TunnelArgs { exit: ExitSelector::Exit { id: "NYC-001".into() } });
    assert!(target_reached(&status, &matching).unwrap());
    let other_exit = Some(TunnelArgs { exit: ExitSelector::Exit { id: "CHI-002".into() } });
    assert!(!target_reached(&status, &other_exit).unwrap());
    let matching_country = Some(TunnelArgs { exit: ExitSelector::Country { country_code: CountryCode("US".into()) } });
    assert!(target_reached(&status, &matching_country).unwrap());
    let other_country = Some(TunnelArgs { exit: ExitSelector::Country { country_code: CountryCode("CA".into()) } });
    assert!(!target_reached(&status, &other_country).unwrap());

    // A disconnected service only satisfies a disconnect target.
    assert!(!target_reached(&VpnStatus::Disconnected {}, &matching).unwrap());
    assert!(target_reached(&VpnStatus::Disconnected {}, &None).unwrap());
}

#[test]
fn cli_target_state_commands() {
    match set_tunnel_args_command(&connect_target()) {
        ManagerCmd::SetTunnelArgs { args, active } => {
            assert_eq!(active, Some(true));
            assert!(matches!(args, Some(TunnelArgs { exit: ExitSelector::Any {} })));
        }
        other => panic!("unexpected command: {other:?}"),
    }
    match set_tunnel_args_command(&None) {
        ManagerCmd::SetTunnelArgs { args, active } => {
            assert_eq!(active, Some(false));
            assert!(args.is_none());
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[tokio::test]
async fn cli_timeout_stops_the_wait_without_resubmitting() {
    let submissions = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&submissions);
    let timeout = Timeout::new(0);
    let submit = run_with_deadline(Some(timeout), OperationStep::Submit, async move {
        counter.fetch_add(1, Ordering::SeqCst);
        Ok(())
    });
    let wait = run_with_deadline(Some(timeout), OperationStep::Wait, std::future::pending::<Result<(), ClientError>>());
    let error = apply_target_state(submit, Some(wait)).await.unwrap_err();
    assert_eq!(error.exit_code(), 6);
    assert_eq!(
        submissions.load(Ordering::SeqCst),
        1,
        "a timed out operation must not send the mutation again"
    );
    assert!(
        error.to_string().contains("waiting for the target state"),
        "the error names the step that ran out of time: {error}"
    );
}

#[tokio::test]
async fn cli_no_wait_skips_the_status_wait() {
    let submissions = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&submissions);
    let submit = async move {
        counter.fetch_add(1, Ordering::SeqCst);
        Ok(())
    };
    let result = apply_target_state(submit, None::<std::future::Pending<Result<(), ClientError>>>).await;
    assert!(result.is_ok());
    assert_eq!(submissions.load(Ordering::SeqCst), 1);
}

#[test]
fn cli_locations_commands() {
    let args = locations_list_args(&["obscura", "locations", "list"]);
    assert!(!args.json);
    assert_eq!(args.timeout, None);

    let args = locations_list_args(&["obscura", "locations", "list", "--json"]);
    assert!(args.json);
    assert_eq!(args.timeout, None);

    let args = locations_list_args(&["obscura", "locations", "list", "--timeout", "30"]);
    assert_eq!(args.timeout, Some(30));

    let args = locations_refresh_args(&["obscura", "locations", "refresh"]);
    assert_eq!(args.timeout, None);

    let args = locations_refresh_args(&["obscura", "locations", "refresh", "--timeout", "60"]);
    assert_eq!(args.timeout, Some(60));

    for invalid in ["0", "3601", "abc", "-1"] {
        assert!(
            Cli::try_parse_from(["obscura", "locations", "list", "--timeout", invalid]).is_err(),
            "locations list --timeout {invalid} must be rejected"
        );
        assert!(
            Cli::try_parse_from(["obscura", "locations", "refresh", "--timeout", invalid]).is_err(),
            "locations refresh --timeout {invalid} must be rejected"
        );
    }
    assert!(Cli::try_parse_from(["obscura", "locations", "list", "--timeout", "3600"]).is_ok());
    assert!(Cli::try_parse_from(["obscura", "locations", "refresh", "--timeout", "3600"]).is_ok());

    assert!(
        Cli::try_parse_from(["obscura", "locations", "list", "surplus"]).is_err(),
        "surplus arguments must be rejected"
    );
    assert!(
        Cli::try_parse_from(["obscura", "locations", "refresh", "surplus"]).is_err(),
        "surplus arguments must be rejected"
    );
    assert!(Cli::try_parse_from(["obscura", "locations"]).is_err(), "locations requires a subcommand");
    assert!(
        Cli::try_parse_from(["obscura", "locations", "unknown"]).is_err(),
        "locations only accepts the known subcommands"
    );
}

#[test]
fn cli_locations_text_output() {
    let catalogue = synthetic_catalogue(vec![
        synthetic_catalogue_exit("NYC-001", "US", "nyc", "New York", "us-nyc-wg-301"),
        synthetic_catalogue_exit("CHI-002", "US", "chi", "Chicago", "us-chi-wg-302"),
        synthetic_catalogue_exit("MUC-003", "DE", "muc", "München", "de-muc-wg-303"),
        synthetic_catalogue_exit("ZRH-004", "CH", "zrh", "Zürich", "ch-zrh-wg-304"),
    ]);

    let mut out = Vec::new();
    write_catalogue_text(&catalogue, &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 4, "every exit is reported: {text}");

    // The list is sorted by country, then city, then exit ID.
    assert!(lines[0].starts_with("CH"), "{text}");
    assert!(lines[1].starts_with("DE"), "{text}");
    assert!(lines[2].starts_with("US  chi"), "{text}");
    assert!(lines[3].starts_with("US  nyc"), "{text}");

    // The exit ID and the provider ID are labeled separately.
    assert!(lines[3].contains("exit NYC-001"), "{text}");
    assert!(lines[3].contains("provider us-nyc-wg-301"), "{text}");

    // A non-ASCII city name is reported as it is.
    assert!(lines[0].contains("Zürich"), "{text}");
    assert!(lines[1].contains("München"), "{text}");
}

#[test]
fn cli_locations_text_output_separates_cities_with_the_same_code() {
    let catalogue = synthetic_catalogue(vec![
        synthetic_catalogue_exit("NYC-001", "US", "nyc", "New York", "us-nyc-wg-301"),
        synthetic_catalogue_exit("NYC-002", "CA", "nyc", "New York", "ca-nyc-wg-302"),
    ]);

    let mut out = Vec::new();
    write_catalogue_text(&catalogue, &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 2, "both exits are reported: {text}");
    assert!(lines[0].starts_with("CA  nyc"), "{text}");
    assert!(lines[1].starts_with("US  nyc"), "{text}");
}

#[test]
fn cli_locations_json_output_is_the_cache_wrapper() {
    let catalogue = synthetic_catalogue(vec![synthetic_catalogue_exit("NYC-001", "US", "nyc", "New York", "us-nyc-wg-301")]);

    let mut out = Vec::new();
    write_catalogue_json(&catalogue, &mut out).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&out).expect("the catalogue must be one JSON value");

    assert!(value.get("version").is_some(), "the cache wrapper carries the version: {value}");
    assert!(value.get("last_updated").is_some(), "the cache wrapper carries the update time: {value}");
    let exits = value
        .get("value")
        .and_then(|value| value.get("exits"))
        .and_then(|exits| exits.as_array())
        .expect("the exits are under value.exits");
    assert_eq!(exits.len(), 1);
    assert_eq!(exits[0].get("id").and_then(|id| id.as_str()), Some("NYC-001"));
    assert!(text_ends_with_newline(&out));
}

#[test]
fn cli_locations_empty_catalogue_names_the_next_step() {
    let catalogue = synthetic_catalogue(Vec::new());

    let mut out = Vec::new();
    write_catalogue_text(&catalogue, &mut out).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.contains("locations refresh"), "an empty catalogue must name the next step: {text}");

    let mut json = Vec::new();
    write_catalogue_json(&catalogue, &mut json).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&json).unwrap();
    let exits = value
        .get("value")
        .and_then(|value| value.get("exits"))
        .and_then(|exits| exits.as_array())
        .expect("the exits are under value.exits");
    assert!(exits.is_empty());
}

#[test]
fn cli_locations_cache_read_and_refresh_are_different_requests() {
    match catalogue_read_command() {
        ManagerCmd::GetExitList { known_version } => {
            assert!(known_version.is_none(), "the cache is read without waiting for a new version");
        }
        other => panic!("unexpected command: {other:?}"),
    }
    match catalogue_refresh_command() {
        ManagerCmd::RefreshExitList { freshness } => {
            assert_eq!(freshness, Duration::ZERO, "the refresh always asks for the current catalogue");
        }
        other => panic!("unexpected command: {other:?}"),
    }
}

#[test]
fn cli_catalogue_errors_are_reported_as_timeouts() {
    assert_eq!(ClientError::CatalogueUnavailable.exit_code(), 6);
    assert_eq!(ClientError::CatalogueRefreshTimedOut { seconds: 30 }.exit_code(), 6);
    assert!(
        ClientError::CatalogueUnavailable.to_string().contains("locations refresh"),
        "an unavailable catalogue must name the next step"
    );
    assert!(
        ClientError::CatalogueRefreshTimedOut { seconds: 30 }
            .to_string()
            .contains("exit catalogue"),
        "a refresh timeout must name what was being refreshed"
    );
}

#[tokio::test]
async fn cli_catalogue_read_reports_an_unavailable_catalogue() {
    let timeout = Timeout::new(0);
    let error = read_catalogue_with(timeout, std::future::pending()).await.unwrap_err();
    assert_eq!(error.exit_code(), 6);
    assert!(
        error.to_string().contains("locations refresh"),
        "an unavailable catalogue must name the next step: {error}"
    );
}

#[tokio::test]
async fn cli_catalogue_refresh_timeout_does_not_retry() {
    let requests = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&requests);
    let timeout = Timeout::new(0);
    let request = async move {
        counter.fetch_add(1, Ordering::SeqCst);
        std::future::pending::<Result<Result<(), ManagerCmdErrorCode>, LinuxIpcError>>().await
    };
    let error = refresh_catalogue_with(timeout, request).await.unwrap_err();
    assert_eq!(error.exit_code(), 6);
    assert_eq!(requests.load(Ordering::SeqCst), 1, "a refresh is sent exactly once");
}

/// Returns true when the output ends with a newline.
fn text_ends_with_newline(output: &[u8]) -> bool {
    output.last() == Some(&b'\n')
}

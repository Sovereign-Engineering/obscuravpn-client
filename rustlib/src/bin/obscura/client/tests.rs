use super::*;
use crate::{Cli, ClientCommand, ClientStatusArgs, Command};
use clap::Parser;
use obscuravpn_client::config::feature_flags::FeatureFlags;
use obscuravpn_client::errors::ConnectErrorCode;
use obscuravpn_client::network_config::DnsContentBlock;
use uuid::Uuid;

fn status_args(args: &[&str]) -> ClientStatusArgs {
    let cli = Cli::try_parse_from(args).expect("failed to parse arguments");
    let Command::Client(ClientCommand::Status(status_args)) = cli.command else {
        panic!("expected a status command, got {:?}", cli.command);
    };
    status_args
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

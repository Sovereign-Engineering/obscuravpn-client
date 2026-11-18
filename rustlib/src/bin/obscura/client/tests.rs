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
    write_account_summary(StatusSource::LocalOnly, Some("active"), true, &mut local).unwrap();
    write_status(&status, true, &mut local).unwrap();
    let mut default = Vec::new();
    write_account_summary(StatusSource::WithAccountRefresh, Some("active"), true, &mut default).unwrap();
    write_status(&status, true, &mut default).unwrap();
    assert_eq!(local, default, "the JSON output must not depend on the account refresh");
    assert!(!String::from_utf8(local).unwrap().contains("Account is"));

    let mut local_text = Vec::new();
    write_account_summary(StatusSource::LocalOnly, Some("active"), false, &mut local_text).unwrap();
    write_status(&status, false, &mut local_text).unwrap();
    let local_text = String::from_utf8(local_text).unwrap();
    assert!(!local_text.contains("Account is"), "the account line must be absent in local mode");
    assert!(local_text.contains("VPN is "));

    let mut default_text = Vec::new();
    write_account_summary(StatusSource::WithAccountRefresh, Some("active"), false, &mut default_text).unwrap();
    write_status(&status, false, &mut default_text).unwrap();
    assert!(String::from_utf8(default_text).unwrap().contains("Account is active."));
}

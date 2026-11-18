use super::*;
use clap::Parser;
use obscuravpn_client::errors::ConnectErrorCode;

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

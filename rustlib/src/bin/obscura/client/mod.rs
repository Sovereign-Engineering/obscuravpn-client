mod client_error;
mod ipc;

use crate::client::client_error::ClientError;
use crate::client::ipc::run_command;
use crate::{ClientCommand, ClientLoginArgs, ClientStatusArgs};
use anyhow::Context;
use chrono::{MappedLocalTime, TimeZone};
use obscuravpn_api::types::{AccountId, AccountInfo};
use obscuravpn_client::exit_selection::ExitSelector;
use obscuravpn_client::manager::{Status, TunnelArgs, VpnStatus};
use obscuravpn_client::manager_cmd::ManagerCmd;

pub async fn run(cmd: ClientCommand) -> Result<(), ClientError> {
    match cmd {
        ClientCommand::Login(args) => login(args).await,
        ClientCommand::Start(_args) => go_to_target_state(Some(TunnelArgs { exit: ExitSelector::Any {} })).await,
        ClientCommand::Stop(_args) => go_to_target_state(None).await,
        ClientCommand::Status(args) => status(args).await,
    }
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
    let _: () = run_command(ManagerCmd::Login { account_id: AccountId::from_string_unchecked(args.account), validate: !args.offline }).await??;
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
    loop {
        let status: Status = run_command(ManagerCmd::GetStatus { known_version }).await??;
        known_version = Some(status.version);
        eprintln!("{}", vpn_status_summary(&status.vpn_status));
        match (&status.vpn_status, &target_state) {
            (VpnStatus::Connected { exit, .. }, Some(TunnelArgs { exit: exit_selector })) if exit_selector.matches(exit) => break,
            (VpnStatus::Disconnected {}, None) => break,
            _ => {}
        }
    }
    eprintln!("reached target state");
    Ok(())
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

use clap::Parser;
use obscuravpn_api::types::AccountId;
use obscuravpn_client::client_state::ClientState;
use obscuravpn_client::exit_selection::{ExitSelectionState, ExitSelector};
use std::time::Duration;
use tokio::time::sleep;

#[derive(Parser, Debug, PartialEq)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Args {
    #[clap(long)]
    base_url: Option<String>,
    #[clap(long)]
    account_no: Option<String>,
    #[clap(long)]
    force_tcp_tls: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install aws-lc crypto provider");

    let args = Args::parse();

    let client_state = ClientState::new(".".into(), None, "list-relays".into(), None)?;
    client_state.set_api_url(args.base_url)?;
    client_state.set_force_tcp_tls_relay_transport(args.force_tcp_tls)?;
    if let Some(account_no) = args.account_no {
        let account_id = AccountId::from_string_unchecked(account_no);
        client_state.set_account_id(Some(account_id), None)?;
    }

    let mut exit_selection_state = ExitSelectionState::default();
    let conn = loop {
        match client_state.connect(&ExitSelector::Any {}, None, &mut exit_selection_state).await {
            Ok((conn, ..)) => break conn,
            Err(error) => tracing::error!("connection attempt failed: {error}"),
        }
        sleep(Duration::from_secs(1)).await;
    };

    tracing::info!("connected");
    loop {
        let packet = conn.receive().await?;
        tracing::info!("received packet with {} bytes", packet.len());
    }
}

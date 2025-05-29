use clap::Parser;
use obscuravpn_api::cmd::ListRelays;
use obscuravpn_api::types::AccountId;
use obscuravpn_client::client_state::ClientState;
use obscuravpn_client::relay_selection::race_relay_handshakes;

#[derive(Parser, Debug, PartialEq)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
struct Args {
    #[clap(long)]
    base_url: Option<String>,
    #[clap(long)]
    account_no: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let args = Args::parse();

    let client_state = ClientState::new(".".into(), "list-relays".into())?;
    client_state.set_api_url(args.base_url)?;
    if let Some(account_no) = args.account_no {
        let account_id = AccountId::from_string_unchecked(account_no);
        client_state.set_account_id(Some(account_id), None)?;
    }
    let relays = client_state.api_request(ListRelays {}).await?;

    let connection_stream = race_relay_handshakes(relays, "relay.example".into())?;
    while let Ok((relay, port, rtt, handshaking)) = connection_stream.recv_async().await {
        println!("{}:{:03} rtt={:03}ms", relay.id, port, rtt.as_millis());
        handshaking.abandon().await;
    }
    Ok(())
}

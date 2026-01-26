use clap::{Args, Parser, Subcommand, ValueEnum};
use derive_more::From;
use std::process::exit;
use tracing_subscriber::EnvFilter;

#[cfg(target_os = "linux")]
mod add_operator;
#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
mod client;
#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
mod service;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum DnsManagerArg {
    Auto,
    Disabled,
    #[cfg(target_os = "linux")]
    NetworkManager,
    #[cfg(target_os = "linux")]
    Resolved,
}

#[derive(Args, Debug)]
pub struct ServiceArgs {
    #[clap(long, default_value = "/var/lib/obscura")]
    pub config_dir: String,
    #[arg(long, value_enum, default_value_t = DnsManagerArg::Auto)]
    pub dns: DnsManagerArg,
}

#[derive(Args, Debug)]
pub struct ClientLoginArgs {
    /// Account number (20 decimal digits without dashes or spaces).
    pub account: String,
    #[clap(long)]
    /// Don't validate the account number, which would require internet access.
    pub offline: bool,
}

#[derive(Args, Debug)]
pub struct ClientStartArgs {}

#[derive(Args, Debug)]
pub struct ClientStopArgs {}

#[derive(Args, Debug)]
pub struct ClientStatusArgs {
    #[arg(long, short)]
    /// Continuously print new status updates as they are published by the service.
    pub follow: bool,
    #[arg(long)]
    /// Print full JSON status instead of summary.
    pub json: bool,
}
#[derive(From)]
pub enum ClientCommand {
    Login(ClientLoginArgs),
    Start(ClientStartArgs),
    Stop(ClientStopArgs),
    Status(ClientStatusArgs),
}

#[derive(Subcommand, Debug)]
pub enum Command {
    #[cfg(target_os = "linux")]
    /// Grant operator privileges by adding the specified users to the 'obscura' group. Defaults to the current user.
    AddOperator {
        users: Vec<String>,
    },
    Service(ServiceArgs),
    Login(ClientLoginArgs),
    Start(ClientStartArgs),
    Stop(ClientStopArgs),
    Status(ClientStatusArgs),
}

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install aws-lc crypto provider");

    let client_command: ClientCommand = match Cli::parse().command {
        #[cfg(target_os = "linux")]
        Command::AddOperator { users } => add_operator::run_add_operator(users).await,
        Command::Service(args) => run_service(args).await,
        Command::Start(args) => args.into(),
        Command::Stop(args) => args.into(),
        Command::Status(args) => args.into(),
        Command::Login(args) => args.into(),
    };
    run_client(client_command).await
}

#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
async fn run_service(args: ServiceArgs) -> ! {
    let Err(error) = service::run(args).await;
    eprintln!("failed to start service: {}", error);
    exit(1)
}

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
async fn run_service(_args: ServiceArgs) -> ! {
    eprintln!("unsupported OS");
    exit(1)
}

#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
async fn run_client(args: ClientCommand) {
    if let Err(error) = client::run(args).await {
        eprintln!("{}", error);
        exit(1)
    }
}

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
async fn run_client(_args: ClientCommand) {
    eprintln!("unsupported OS");
    exit(1)
}

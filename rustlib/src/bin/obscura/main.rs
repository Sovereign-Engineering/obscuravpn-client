use clap::{Args, Parser, Subcommand};
use derive_more::From;
use std::process::exit;
use strum::EnumIs;
use tracing_subscriber::EnvFilter;

#[cfg(target_os = "linux")]
mod add_operator;
#[cfg(target_os = "linux")]
mod client;
#[cfg(any(target_os = "windows", target_os = "linux"))]
mod service;

#[cfg(not(target_os = "windows"))]
fn get_data_dir() -> String {
    "/var/lib/obscura".to_string()
}

#[cfg(target_os = "windows")]
fn get_data_dir() -> String {
    use standard_paths::{LocationType, StandardPaths};

    let sp = StandardPaths::new("Obscura", "");
    sp.writable_location(LocationType::AppDataLocation)
        .expect("failed to determine config directory")
        .to_string_lossy()
        .into_owned()
}

#[derive(Args, Debug)]
pub struct ServiceArgs {
    #[clap(long, default_value_t = get_data_dir())]
    pub config_dir: String,
    #[cfg(target_os = "linux")]
    #[arg(long, value_enum, default_value_t = service::os::linux::dns::DnsManagerArg::Auto)]
    pub dns: service::os::linux::dns::DnsManagerArg,
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

#[derive(Args, Debug)]
pub struct ClientIpcTestArgs {}

#[derive(From, EnumIs)]
pub enum ClientCommand {
    Login(ClientLoginArgs),
    Start(ClientStartArgs),
    Stop(ClientStopArgs),
    Status(ClientStatusArgs),
    IpcTest(ClientIpcTestArgs),
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
    #[command(hide = true)]
    IpcTest(ClientIpcTestArgs),
}

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
    #[command(flatten)]
    pub global_args: GlobalArgs,
}

#[derive(Args, Debug)]
pub struct GlobalArgs {
    #[clap(long, hide = true)]
    no_group_refresh: bool,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install aws-lc crypto provider");

    let cli = Cli::parse();
    let client_command: ClientCommand = match cli.command {
        #[cfg(target_os = "linux")]
        Command::AddOperator { users } => add_operator::run_add_operator(users).await,
        Command::Service(args) => run_service(args).await,
        Command::Start(args) => args.into(),
        Command::Stop(args) => args.into(),
        Command::Status(args) => args.into(),
        Command::Login(args) => args.into(),
        Command::IpcTest(args) => args.into(),
    };
    run_client(cli.global_args, client_command).await
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
async fn run_service(args: ServiceArgs) -> ! {
    let Err(error) = service::run(args).await;
    eprintln!("failed to start service: {}", error);
    exit(1)
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
async fn run_service(_args: ServiceArgs) -> ! {
    eprintln!("unsupported OS");
    exit(1)
}

#[cfg(target_os = "linux")]
async fn run_client(global_args: GlobalArgs, args: ClientCommand) {
    if let Err(error) = client::run(global_args, args).await {
        eprintln!("{}", error);
        exit(1)
    }
}

#[cfg(not(target_os = "linux"))]
async fn run_client(_global_args: GlobalArgs, _args: ClientCommand) {
    eprintln!("unsupported OS");
    exit(1)
}

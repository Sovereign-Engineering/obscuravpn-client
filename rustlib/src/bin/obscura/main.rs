use camino::Utf8PathBuf;
use clap::{ArgAction, Args, Parser, Subcommand};
use obscuravpn_client::logging::{self, LogPersistence};
use std::process::exit;
use std::time::Duration;
use tracing_subscriber::filter::{EnvFilter, LevelFilter};
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::{Layer, Registry, fmt};

#[cfg(target_os = "linux")]
mod add_operator;
#[cfg(target_os = "linux")]
mod client;
#[cfg(any(target_os = "windows", target_os = "linux"))]
mod service;

#[cfg(target_os = "windows")]
fn get_data_dir() -> String {
    use standard_paths::{LocationType, StandardPaths};

    let sp = StandardPaths::new("Obscura", "");
    sp.writable_location(LocationType::AppLocalDataLocation)
        .expect("failed to determine config directory")
        .to_string_lossy()
        .into_owned()
}

#[derive(Args, Debug)]
pub struct ServiceArgs {
    #[cfg(any(target_os = "windows", target_os = "linux"))]
    #[cfg_attr(target_os = "linux", clap(env = "STATE_DIRECTORY"))]
    #[cfg_attr(target_os = "windows", clap(default_value_t = get_data_dir()))]
    #[clap(long)]
    pub config_dir: String,
    #[cfg(target_os = "linux")]
    #[clap(long, env = "LOGS_DIRECTORY")]
    pub log_dir: String,
    #[cfg_attr(target_os = "linux", clap(env = "RUNTIME_DIRECTORY"))]
    #[clap(long)]
    pub runtime_dir: Option<String>,
    #[cfg(target_os = "linux")]
    #[arg(long, value_enum, default_value_t = service::os::linux::dns::DnsManagerArg::Auto)]
    pub dns: service::os::linux::dns::DnsManagerArg,
}

#[derive(Args, Debug)]
pub struct ClientLoginArgs {
    /// Account number. You will be prompted for it if omitted.
    pub account: Option<String>,
    #[clap(long)]
    /// Don't validate the account number, which would require internet access.
    pub offline: bool,
}

#[derive(Args, Debug)]
pub struct ClientConnectArgs {}

#[derive(Args, Debug)]
pub struct ClientDisconnectArgs {}

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

#[derive(Args, Debug)]
pub struct ClientDebugBundleArgs {
    /// Message for the Obscura team, included in the debug bundle.
    pub feedback: String,
}

#[derive(Subcommand, Debug)]
pub enum ClientCommand {
    #[cfg(target_os = "linux")]
    /// Grant operator privileges by adding the specified users to the 'obscura' group. Defaults to the current user.
    AddOperator { users: Vec<String> },
    /// Log in with your account number.
    Login(ClientLoginArgs),
    /// Connect to the VPN.
    Connect(ClientConnectArgs),
    /// Disconnect from the VPN.
    Disconnect(ClientDisconnectArgs),
    /// Show account and VPN status.
    Status(ClientStatusArgs),
    #[cfg(target_os = "linux")]
    /// Create a debug bundle and print its path.
    DebugBundle(ClientDebugBundleArgs),
    #[command(hide = true)]
    IpcTest(ClientIpcTestArgs),
}

impl ClientCommand {
    fn init_logging(&self, verbosity: u8) {
        let level = match verbosity {
            0 => LevelFilter::OFF,
            1 => LevelFilter::ERROR,
            2 => LevelFilter::INFO,
            _ => LevelFilter::TRACE,
        };
        let mut stderr_filter = EnvFilter::builder().with_default_directive(level.into()).from_env_lossy();
        let env_level = <EnvFilter as Layer<Registry>>::max_level_hint(&stderr_filter);
        if env_level.is_none_or(|env_level| env_level < level) {
            stderr_filter = stderr_filter.add_directive(level.into());
        }
        let stderr_layer = fmt::Layer::default().with_writer(std::io::stderr).with_filter(stderr_filter);
        let registry = tracing_subscriber::registry().with(stderr_layer);
        #[cfg(target_os = "linux")]
        let registry = registry.with(tracing_journald::Layer::new().map(|layer| layer.with_filter(LevelFilter::INFO)).ok());
        tracing::subscriber::set_global_default(registry).expect("failed to set global subscriber");
    }
}

#[derive(Subcommand, Debug)]
pub enum ServiceCommand {
    #[command(hide = true)]
    Service(ServiceArgs),
    #[cfg(target_os = "windows")]
    #[command(hide = true)]
    WindowsService(ServiceArgs),
}

#[derive(Subcommand, Debug)]
pub enum Command {
    #[command(flatten)]
    Service(ServiceCommand),
    #[command(flatten)]
    Client(ClientCommand),
}

impl ServiceCommand {
    fn log_persistence_dir(&self) -> Option<Utf8PathBuf> {
        let dir = match self {
            #[cfg(target_os = "linux")]
            Self::Service(ServiceArgs { log_dir, .. }) => Some(Utf8PathBuf::from(log_dir)),
            #[cfg(target_os = "windows")]
            Self::Service(ServiceArgs { config_dir, .. }) | Self::WindowsService(ServiceArgs { config_dir, .. }) => {
                Some(Utf8PathBuf::from_iter([config_dir.as_str(), "logs"]))
            }
            #[cfg(not(any(target_os = "windows", target_os = "linux")))]
            Self::Service(ServiceArgs { .. }) => None,
        }?;
        if let Err(error) = std::fs::create_dir_all(&dir) {
            eprintln!("failed to create log dir {dir}: {error}");
        }
        Some(dir)
    }

    fn init_logging(&self) -> Option<LogPersistence> {
        let persistence_dir = self.log_persistence_dir();
        #[cfg(target_os = "linux")]
        let base_layer: Box<dyn Layer<Registry> + Send + Sync> = match std::env::var_os("JOURNAL_STREAM").map(|_| tracing_journald::Layer::new()) {
            Some(Ok(layer)) => Box::new(layer),
            Some(Err(_)) | None => Box::new(fmt::Layer::default()),
        };
        #[cfg(not(target_os = "linux"))]
        let base_layer: Box<dyn Layer<Registry> + Send + Sync> = Box::new(fmt::Layer::default());
        logging::init(base_layer, persistence_dir.as_deref())
    }
}

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
    #[clap(long, hide = true, global = true)]
    no_group_refresh: bool,
    /// Extra logs (-v: errors, -vv: info, -vvv: everything).
    #[clap(short, long, action = ArgAction::Count, global = true)]
    verbose: u8,
}

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(1);

fn main() {
    let runtime = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    runtime.block_on(async_main());
    runtime.shutdown_timeout(SHUTDOWN_TIMEOUT);
}

async fn async_main() {
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install aws-lc crypto provider");

    let cli = Cli::parse();
    match cli.command {
        Command::Client(command) => {
            command.init_logging(cli.verbose);
            run_client(cli.no_group_refresh, command).await
        }
        Command::Service(command) => {
            let log_persistence = command.init_logging();
            match command {
                ServiceCommand::Service(args) => run_service(args, log_persistence).await,
                #[cfg(target_os = "windows")]
                ServiceCommand::WindowsService(args) => {
                    if let Err(error) = service::os::windows::scm::run(args.config_dir.clone(), log_persistence) {
                        eprintln!("failed to run as windows service: {}", error);
                        exit(1);
                    }
                }
            }
        }
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
async fn run_service(args: ServiceArgs, log_persistence: Option<LogPersistence>) -> ! {
    // Foreground `service` runs until the process is terminated; the Windows service path supplies a real stop signal and an SCM start reason.
    match service::run(args, log_persistence, None, None).await {
        Ok(()) => exit(0),
        Err(error) => {
            eprintln!("failed to start service: {}", error);
            exit(1)
        }
    }
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
async fn run_service(_args: ServiceArgs, _log_persistence: Option<LogPersistence>) -> ! {
    eprintln!("unsupported OS");
    exit(1)
}

#[cfg(target_os = "linux")]
async fn run_client(no_group_refresh: bool, args: ClientCommand) {
    if let Err(error) = client::run(no_group_refresh, args).await {
        eprintln!("{}", error);
        exit(1)
    }
}

#[cfg(not(target_os = "linux"))]
async fn run_client(_no_group_refresh: bool, _args: ClientCommand) {
    eprintln!("unsupported OS");
    exit(1)
}

use clap::{Args, Parser, Subcommand, ValueEnum};
use std::convert::Infallible;

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
    #[clap(long)]
    pub connect: bool,
    #[clap(long)]
    pub account: Option<String>,
    #[clap(long)]
    pub config_dir: String,
    #[arg(long, value_enum, default_value_t = DnsManagerArg::Auto)]
    pub dns: DnsManagerArg,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Service(ServiceArgs),
}

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

fn main() -> anyhow::Result<Infallible> {
    tracing_subscriber::fmt().with_env_filter("info").init();
    tracing::info!("starting up");

    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("Failed to install aws-lc crypto provider");

    match Cli::parse().command {
        Command::Service(args) => run_service(args),
    }
}

#[cfg(not(any(target_os = "macos", target_os = "ios", target_os = "android")))]
fn run_service(args: ServiceArgs) -> anyhow::Result<Infallible> {
    service::run(args)
}

#[cfg(any(target_os = "macos", target_os = "ios", target_os = "android"))]
fn run_service(_args: ServiceArgs) -> anyhow::Result<Infallible> {
    anyhow::bail!("unsupported OS");
}

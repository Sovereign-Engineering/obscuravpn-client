mod error;
mod fix;
mod wip;

use clap::{Parser, Subcommand};
use obscuravpn_client::linux::argv0;
use obscuravpn_client::linux::exit_list_watch::GuiExitListWatch;
use obscuravpn_client::linux::ipc::{run_command, try_group_refresh_fix};
use obscuravpn_client::linux::status_watch::GuiStatusWatch;
use obscuravpn_client::linux::tray::spawn_tray;
use obscuravpn_client::manager_cmd::ManagerCmd;
use obscuravpn_client::version::release_version;
use std::marker::PhantomData;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::process::ExitCode;

#[derive(Parser)]
struct GuiArgs {
    #[arg(long, hide = true)]
    no_group_refresh: bool,
    #[arg(long, help = "Print version")]
    version: bool,
    #[command(subcommand)]
    command: Option<GuiCommand>,
}

#[derive(Subcommand)]
enum GuiCommand {
    #[command(hide = true)]
    IpcTest,
    #[command(hide = true)]
    Version,
    #[command(hide = true)]
    RunGui,
}

/// Proof of running on the main thread. Neither Send nor Sync, so it cannot leave the thread it was created on.
#[derive(Clone, Copy)]
pub(crate) struct MainThreadToken {
    _not_send_sync: PhantomData<*const ()>,
}

static_assertions::assert_not_impl_any!(MainThreadToken: Send, Sync);

impl MainThreadToken {
    /// Must only be called on the main thread.
    fn assert() -> Self {
        Self { _not_send_sync: PhantomData }
    }
}

pub(crate) enum GtkAppFinished {
    Exit(ExitCode),
    Restart,
}

fn main() -> ExitCode {
    let main_thread = MainThreadToken::assert();
    let runtime = tokio::runtime::Runtime::new().expect("failed to initialize tokio runtime");
    let _runtime_guard = runtime.enter();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::builder()
                .with_default_directive(tracing::level_filters::LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let GuiArgs { no_group_refresh, version, command } = GuiArgs::parse();

    if version {
        println!("{}", release_version());
        return ExitCode::SUCCESS;
    }

    match command.unwrap_or(GuiCommand::RunGui) {
        GuiCommand::IpcTest => ipc_test(),
        GuiCommand::Version => {
            println!("{}", release_version());
            ExitCode::SUCCESS
        }
        GuiCommand::RunGui => run_gui(main_thread, no_group_refresh),
    }
}

fn run_gui(main_thread: MainThreadToken, no_group_refresh: bool) -> ExitCode {
    let (gui_status, tray_receiver) = tokio::runtime::Handle::current().block_on(async {
        if !no_group_refresh {
            try_group_refresh_fix().await;
        }
        let gui_status = GuiStatusWatch::watch().await;
        let exit_list = GuiExitListWatch::watch().await;
        let tray_receiver = spawn_tray(gui_status.clone(), exit_list).await;
        (gui_status, tray_receiver)
    });

    match wip::run_gtk_app(main_thread, gui_status, tray_receiver) {
        GtkAppFinished::Exit(exit_code) => exit_code,
        GtkAppFinished::Restart => restart(),
    }
}

fn ipc_test() -> ExitCode {
    match tokio::runtime::Handle::current().block_on(run_command::<()>(ManagerCmd::Ping {})) {
        Ok(Ok(())) => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

fn restart() -> ExitCode {
    let Some(invocation_path) = argv0() else {
        tracing::error!(message_id = "qL8vBn3W", "cannot restart GUI without argv[0]");
        return ExitCode::FAILURE;
    };
    tracing::info!(message_id = "Vt3mHs8B", ?invocation_path, "restarting GUI");
    let error = Command::new(&invocation_path).exec();
    tracing::error!(message_id = "nK6rYw2P", ?error, "failed to restart GUI: {error}");
    ExitCode::FAILURE
}

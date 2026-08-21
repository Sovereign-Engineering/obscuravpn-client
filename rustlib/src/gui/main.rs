mod error;
mod fix;
mod wip;

use camino::Utf8PathBuf;
use clap::{Parser, Subcommand};
use nix::fcntl::{Flock, FlockArg};
use obscuravpn_client::linux::argv0;
use obscuravpn_client::linux::client_log_dir;
use obscuravpn_client::linux::debug_bundle::GuiDebugBundler;
use obscuravpn_client::linux::exit_list_watch::GuiExitListWatch;
use obscuravpn_client::linux::ipc::{run_command, try_group_refresh_fix};
use obscuravpn_client::linux::status_watch::GuiStatusWatch;
use obscuravpn_client::linux::tray::spawn_tray;
use obscuravpn_client::logging::{self, LogPersistence};
use obscuravpn_client::manager_cmd::ManagerCmd;
use obscuravpn_client::version::release_version;
use std::marker::PhantomData;
use std::os::unix::process::CommandExt;
use std::process::Command;
use std::process::ExitCode;
use std::sync::Arc;

#[derive(Parser)]
struct GuiArgs {
    #[arg(long, hide = true, global = true)]
    no_group_refresh: bool,
    #[arg(long, help = "Use in autostart entries")]
    xdg_autostart: bool,
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

    let GuiArgs { no_group_refresh, xdg_autostart: _, version, command } = GuiArgs::parse();
    let command = if version {
        GuiCommand::Version
    } else {
        command.unwrap_or(GuiCommand::RunGui)
    };
    let (_log_lock, _log_persistence, log_dir) = command.init_logging();

    match command {
        GuiCommand::IpcTest => ipc_test(),
        GuiCommand::Version => {
            println!("{}", release_version());
            ExitCode::SUCCESS
        }
        GuiCommand::RunGui => run_gui(main_thread, no_group_refresh, log_dir),
    }
}

impl GuiCommand {
    fn init_logging(&self) -> (Option<Flock<std::fs::File>>, Option<LogPersistence>, Option<Utf8PathBuf>) {
        let persistence = match self {
            GuiCommand::RunGui => true,
            GuiCommand::IpcTest | GuiCommand::Version => false,
        };
        let mut log_dir = None;
        if persistence && let Some(dir) = client_log_dir() {
            if let Err(error) = std::fs::create_dir_all(&dir) {
                eprintln!("failed to create log dir {dir}: {error}");
            }
            log_dir = Some(dir);
        }

        let mut log_lock = None;
        if let Some(dir) = &log_dir {
            match std::fs::File::open(dir) {
                // An already held lock means another GUI instance is running. GTK uniqueness handling will activate it and this one exits.
                Ok(file) => match Flock::lock(file, FlockArg::LockExclusiveNonblock) {
                    Ok(lock) => log_lock = Some(lock),
                    Err((_, errno)) => eprintln!("not persisting logs, log dir {dir} is locked: {errno}"),
                },
                Err(error) => eprintln!("failed to open log dir {dir} for locking: {error}"),
            }
        }

        let persist_dir = if log_lock.is_some() { log_dir.as_deref() } else { None };
        let log_persistence = logging::init(tracing_subscriber::fmt::Layer::default(), persist_dir);
        (log_lock, log_persistence, log_dir)
    }
}

fn run_gui(main_thread: MainThreadToken, no_group_refresh: bool, log_dir: Option<Utf8PathBuf>) -> ExitCode {
    let debug_bundler = Arc::new(GuiDebugBundler::new(log_dir));
    let (gui_status, tray_receiver) = tokio::runtime::Handle::current().block_on(async {
        if !no_group_refresh {
            try_group_refresh_fix().await;
        }
        let gui_status = GuiStatusWatch::watch(debug_bundler.subscribe()).await;
        let exit_list = GuiExitListWatch::watch().await;
        let tray_receiver = spawn_tray(gui_status.clone(), exit_list).await;
        (gui_status, tray_receiver)
    });

    match wip::run_gtk_app(main_thread, gui_status, tray_receiver, debug_bundler) {
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

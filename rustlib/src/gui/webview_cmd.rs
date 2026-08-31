use crate::error::LinuxErrorCode;
use crate::fix::{add_operator, restart_service};
use obscuravpn_client::linux::autostart;
use obscuravpn_client::linux::debug_bundle::{DebugBundleError, GuiDebugBundler};
use obscuravpn_client::linux::file_manager::reveal_item_in_dir;
use obscuravpn_client::linux::ipc::run_command;
use obscuravpn_client::linux::status::NavigationView;
use obscuravpn_client::linux::status_watch::GuiStatusWatch;
use obscuravpn_client::manager::TunnelArgs;
use obscuravpn_client::manager_cmd::ManagerCmd;
use obscuravpn_client::ui_config::{ColorScheme, UiConfigHandle};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;
use webkit6::javascriptcore;

#[derive(Clone)]
pub(crate) struct WebviewCmdContext {
    pub(crate) gui_status: Arc<GuiStatusWatch>,
    pub(crate) debug_bundler: Arc<GuiDebugBundler>,
    pub(crate) ui_config: Arc<UiConfigHandle>,
    pub(crate) color_scheme: tokio::sync::watch::Sender<ColorScheme>,
    pub(crate) restart: tokio::sync::watch::Sender<bool>,
    pub(crate) page_ready: tokio::sync::watch::Sender<bool>,
}

// Ref: https://github.com/Sovereign-Engineering/obscuravpn-client-internal/blob/50ae1ec989463f1ff2a5b7ee12d11f58a1de5c1a/apple/client/command.swift#L9-L33
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
enum WebviewCmd {
    DebugBundle { user_feedback: String },
    GetOsStatus { known_version: Option<Uuid> },
    SetColorScheme { value: ColorScheme },
    SetNavigationView { view: NavigationView },
    JsonFfiCmd { cmd: String, timeout_ms: Option<serde_json::Number> },
    StartTunnel { tunnel_args: String },
    StopTunnel {},
    RevealItemInDir { path: String },
    RestartService { enable: bool },
    LinuxAddOperator {},
    RestartApp {},
    RegisterAsLoginItem {},
    UnregisterAsLoginItem {},
    RefreshLoginItemStatus {},
}

const EMPTY_OBJECT: &str = "{}";

impl WebviewCmdContext {
    pub(crate) async fn handle_command_json(self, value: javascriptcore::Value, reply: webkit6::ScriptMessageReply) {
        let command_json = value.to_str();

        let Some(value_context) = value.context() else {
            tracing::error!(message_id = "Qn5bWf8J", "webview command value has no context");
            reply.return_error_message(LinuxErrorCode::Other.as_static_str());
            return;
        };

        let cmd: WebviewCmd = match serde_json::from_str(command_json.as_str()) {
            Ok(cmd) => cmd,
            Err(error) => {
                tracing::error!(message_id = "Xv2hRp6C", %error, command = command_json.as_str(), "failed to parse webview command");
                reply.return_error_message(LinuxErrorCode::Other.as_static_str());
                return;
            }
        };
        tracing::debug!(message_id = "Ez7tKm3B", ?cmd, "handling webview command");

        match tokio::spawn(cmd.run(self)).await {
            Ok(Ok(json)) => reply.return_value(&javascriptcore::Value::new_string(&value_context, Some(&json))),
            Ok(Err(error)) => reply.return_error_message(error.as_static_str()),
            Err(error) => {
                tracing::error!(message_id = "Jf4wQn8T", %error, "webview command task failed");
                reply.return_error_message(LinuxErrorCode::Other.as_static_str());
            }
        }
    }

    async fn login_item_command(&self, action: impl Future<Output = Result<(), ()>>) -> Result<String, LinuxErrorCode> {
        let result = action.await;
        self.gui_status.refresh_login_item_status().await;
        result.map_err(|()| LinuxErrorCode::Other)?;
        Ok(EMPTY_OBJECT.to_owned())
    }
}

impl WebviewCmd {
    async fn run(self, context: WebviewCmdContext) -> Result<String, LinuxErrorCode> {
        match self {
            WebviewCmd::DebugBundle { user_feedback } => {
                let path = context.debug_bundler.create(user_feedback).await.map_err(|error| match error {
                    DebugBundleError::InProgress => LinuxErrorCode::DebugBundleInProgress,
                    DebugBundleError::Failed => LinuxErrorCode::Other,
                })?;
                let _ = reveal_item_in_dir(path.as_str()).await;
                to_json(&path)
            }
            WebviewCmd::GetOsStatus { known_version } => {
                context.page_ready.send_replace(true);
                to_json(&context.gui_status.changed(known_version).await)
            }
            WebviewCmd::SetColorScheme { value } => {
                context.color_scheme.send_replace(value);
                context.ui_config.update(move |ui_config| ui_config.color_scheme = value).await;
                Ok(EMPTY_OBJECT.to_owned())
            }
            WebviewCmd::SetNavigationView { view } => {
                context.gui_status.set_navigation_view(view);
                Ok(EMPTY_OBJECT.to_owned())
            }
            WebviewCmd::JsonFfiCmd { cmd, timeout_ms } => {
                let mgr_cmd: ManagerCmd = match serde_json::from_str(&cmd) {
                    Ok(mgr_cmd) => mgr_cmd,
                    Err(error) => {
                        tracing::error!(message_id = "Wn6cVd3Q", %error, %cmd, "failed to parse ffi command");
                        return Err(LinuxErrorCode::Other);
                    }
                };
                let fut = run_command::<Box<RawValue>>(mgr_cmd);
                let response = match timeout_ms {
                    Some(timeout_ms) => {
                        let Some(timeout_ms) = timeout_ms.as_u64() else {
                            tracing::error!(message_id = "uW2fJn9K", %timeout_ms, "timeout_ms cannot be represented as u64");
                            return Err(LinuxErrorCode::Other);
                        };
                        match tokio::time::timeout(Duration::from_millis(timeout_ms), fut).await {
                            Ok(response) => response,
                            Err(error) => {
                                tracing::error!(message_id = "oM5xDt3V", %error, timeout_ms, "manager command timed out");
                                return Err(LinuxErrorCode::Other);
                            }
                        }
                    }
                    None => fut.await,
                };
                match response {
                    Ok(Ok(ok)) => Ok(ok.get().to_owned()),
                    Ok(Err(error)) => {
                        tracing::error!(message_id = "gK8cQb4R", ?error, "manager command failed");
                        Err(error.into())
                    }
                    Err(error) => {
                        tracing::error!(message_id = "wZ3hSp7L", ?error, "failed to run manager command over IPC");
                        Err(LinuxErrorCode::from(&error))
                    }
                }
            }
            WebviewCmd::StartTunnel { tunnel_args } => {
                let args: TunnelArgs = serde_json::from_str(&tunnel_args).map_err(|error| {
                    tracing::error!(message_id = "Nc8dVt2Q", %error, "failed to parse tunnel args");
                    LinuxErrorCode::Other
                })?;
                let cmd = ManagerCmd::SetTunnelArgs { args: Some(args), active: Some(true) };
                let mut result = set_tunnel_args(cmd.clone()).await;
                // Fail closed: retry transport errors so starts survive unfortunately timed service restarts.
                for _attempt in 1..10 {
                    if !matches!(result, Err(LinuxErrorCode::Ipc(_) | LinuxErrorCode::Other)) {
                        break;
                    }
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    result = set_tunnel_args(cmd.clone()).await;
                }
                result
            }
            WebviewCmd::StopTunnel {} => set_tunnel_args(ManagerCmd::SetTunnelArgs { args: None, active: Some(false) }).await,
            WebviewCmd::RevealItemInDir { path } => {
                let _ = reveal_item_in_dir(&path).await;
                Ok(EMPTY_OBJECT.to_owned())
            }
            WebviewCmd::RestartService { enable } => {
                restart_service(enable).await.map_err(LinuxErrorCode::from)?;
                Ok(EMPTY_OBJECT.to_owned())
            }
            WebviewCmd::LinuxAddOperator {} => {
                add_operator().await.map_err(LinuxErrorCode::from)?;
                context.restart.send_replace(true);
                Ok(EMPTY_OBJECT.to_owned())
            }
            WebviewCmd::RestartApp {} => {
                context.restart.send_replace(true);
                Ok(EMPTY_OBJECT.to_owned())
            }
            WebviewCmd::RegisterAsLoginItem {} => context.login_item_command(autostart::register_autostart()).await,
            WebviewCmd::UnregisterAsLoginItem {} => context.login_item_command(autostart::unregister_autostart()).await,
            WebviewCmd::RefreshLoginItemStatus {} => context.login_item_command(async { Ok(()) }).await,
        }
    }
}

async fn set_tunnel_args(cmd: ManagerCmd) -> Result<String, LinuxErrorCode> {
    match run_command::<Box<RawValue>>(cmd).await {
        Ok(Ok(ok)) => Ok(ok.get().to_owned()),
        Ok(Err(error)) => {
            tracing::error!(message_id = "Hd6sJw9F", ?error, "set tunnel args failed");
            Err(error.into())
        }
        Err(error) => {
            tracing::error!(message_id = "Tk3pGx5Z", ?error, "failed to set tunnel args over IPC");
            Err(LinuxErrorCode::from(&error))
        }
    }
}

fn to_json(value: &impl Serialize) -> Result<String, LinuxErrorCode> {
    serde_json::to_string(value).map_err(|error| {
        tracing::error!(message_id = "Bw4mYc7S", %error, "failed to serialize command reply");
        LinuxErrorCode::Other
    })
}

// ============================================================================
// TODO: REFACTOR EVERYTHING IN HERE AND REMOVE THIS FILE
// ============================================================================
#![allow(unused_imports)]
#![allow(clippy::enum_variant_names)]

use crate::error::{LinuxErrorCode, LinuxFixErrorCode};
use crate::fix::{add_operator, restart_service};
use crate::{GtkAppFinished, MainThreadToken, auto_connect};
use futures::StreamExt;
use futures::channel::mpsc::{Receiver, UnboundedReceiver, UnboundedSender, unbounded};
use gtk4::gio::{DBusError, DBusProxy, ResourceLookupFlags};
use gtk4::glib::translate::ToGlibPtr as _;
use libadwaita::StyleManager;
use obscuravpn_client::exit_selection::ExitSelector;
use obscuravpn_client::linux::autostart;
use obscuravpn_client::linux::debug_bundle::{DebugBundleError, GuiDebugBundler};
use obscuravpn_client::linux::file_manager::reveal_item_in_dir;
use obscuravpn_client::linux::ipc::run_command;
use obscuravpn_client::linux::status::{NavigationView, OsStatus};
use obscuravpn_client::linux::status_watch::GuiStatusWatch;
use obscuravpn_client::linux::tray::{ShowTarget, TrayRequest};
use obscuravpn_client::manager::{self, TunnelArgs};
use obscuravpn_client::manager_cmd::{ManagerCmd, ManagerCmdErrorCode, ManagerCmdOk};
use obscuravpn_client::ui_config::{ColorScheme, UiConfigHandle};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::future::Future;
use std::process::ExitCode;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};
use std::time::Duration;
use strum::IntoEnumIterator;
use uuid::Uuid;
use webkit6::gdk;
use webkit6::glib;
use webkit6::gtk::{self, Align, Label, ListBox, Orientation, SelectionMode, Stack, StackSidebar, Widget, prelude::*};
use webkit6::{
    HardwareAccelerationPolicy, Settings, URISchemeRequest, UserContentInjectedFrames, UserScript, WebContext, WebView, gio, javascriptcore,
    prelude::*,
};
use zbus_systemd::zbus; // provides the spawn method

fn navigation_split_view(
    gui_status: Arc<GuiStatusWatch>,
    restart: Rc<RefCell<Option<tokio::sync::oneshot::Sender<()>>>>,
    dev_visible: Rc<Cell<bool>>,
    debug_bundler: Arc<GuiDebugBundler>,
    webview_signals: UnboundedSender<WebviewSignal>,
    ui_config: Arc<UiConfigHandle>,
) -> (gtk::Box, ListBox, WebView) {
    let split_view = gtk::Box::new(Orientation::Horizontal, 0);

    let webview = webview(gui_status.clone(), restart, debug_bundler, webview_signals, ui_config);
    webview.set_hexpand(true);

    let sidebar = sidebar(gui_status, dev_visible);

    split_view.append(&sidebar);
    split_view.append(&webview);

    (split_view, sidebar, webview)
}

const JS_ERROR_CAPTURE: &str = r#"
window.onerror = (message, source, lineno, colno, error) => {
    window.webkit.messageHandlers.errorBridge.postMessage(JSON.stringify({
      message: message,
      source: source,
      lineno: lineno,
      colno: colno,
    }, undefined, "\t"));
};
window.onunhandledrejection = (event) => {
    console.error("unhandled promise rejection", event.reason)
}
"#;
const JS_LOG_CAPTURE: &str = r#"
function log(type, msg, ...args) {
    let formatted = [type, msg, ...args.map(a => JSON.stringify(a, undefined, "\t"))].join(" ");
    window.webkit.messageHandlers.logBridge.postMessage(formatted);
}
console.debug = log.bind(null, "debug:");
console.log = log.bind(null, "log:");
console.warn = log.bind(null, "warn:");
console.error = log.bind(null, "error:");
"#;

fn uri_handler(request: &URISchemeRequest) {
    let uri = request.uri().unwrap().to_string();
    eprintln!("handling URI: {uri}");

    let scheme = request.scheme().unwrap();
    eprintln!("handling scheme: {scheme}");

    let path = uri.strip_prefix("web-ui://").unwrap();
    eprintln!("handling path: {path}");

    let rpath = format!("resource:///com/obscura/vpn/web-ui{path}");
    let gfile = gio::File::for_uri(&rpath);
    let info = gfile
        .query_info(
            gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
            gio::FileQueryInfoFlags::NONE,
            gio::Cancellable::NONE,
        )
        .unwrap();
    let mimetype = info.content_type().unwrap();
    eprintln!("Info: {info:?}");
    eprintln!("CType: {mimetype:?}");

    let stream = gfile.read(gio::Cancellable::NONE).unwrap();

    request.finish(&stream, -1, Some(&mimetype));
}

fn webview(
    gui_status: Arc<GuiStatusWatch>,
    restart: Rc<RefCell<Option<tokio::sync::oneshot::Sender<()>>>>,
    debug_bundler: Arc<GuiDebugBundler>,
    webview_signals: UnboundedSender<WebviewSignal>,
    ui_config: Arc<UiConfigHandle>,
) -> WebView {
    let user_content_manager = webkit6::UserContentManager::new();

    let error_capture_script = UserScript::new(
        JS_ERROR_CAPTURE,
        UserContentInjectedFrames::AllFrames,
        webkit6::UserScriptInjectionTime::Start,
        &[],
        &[],
    );
    user_content_manager.add_script(&error_capture_script);
    let log_capture_script = UserScript::new(
        JS_LOG_CAPTURE,
        UserContentInjectedFrames::AllFrames,
        webkit6::UserScriptInjectionTime::Start,
        &[],
        &[],
    );
    user_content_manager.add_script(&log_capture_script);

    user_content_manager.connect_script_message_with_reply_received(Some("commandBridge"), {
        let webview_signals = webview_signals.clone();
        move |ucm, value, reply| {
            let _ = webview_signals.unbounded_send(WebviewSignal::CommandReceived);
            command_bridge(
                ucm,
                value,
                reply,
                gui_status.clone(),
                restart.clone(),
                debug_bundler.clone(),
                ui_config.clone(),
            )
        }
    });
    user_content_manager.register_script_message_handler_with_reply("commandBridge", None);

    user_content_manager.connect_script_message_received(Some("errorBridge"), error_handler);
    user_content_manager.register_script_message_handler("errorBridge", None);

    user_content_manager.connect_script_message_received(Some("logBridge"), log_handler);
    user_content_manager.register_script_message_handler("logBridge", None);

    let settings = Settings::builder()
        .enable_developer_extras(true)
        .allow_universal_access_from_file_urls(true)
        .allow_file_access_from_file_urls(true)
        .hardware_acceleration_policy(HardwareAccelerationPolicy::Never)
        .build();

    let context = WebContext::new();
    context.register_uri_scheme("web-ui", uri_handler);

    let webview = WebView::builder()
        .settings(&settings)
        .user_content_manager(&user_content_manager)
        .web_context(&context)
        .build();

    match option_env!("LOAD_DEV_SERVER") {
        None | Some("") => {
            webview.load_uri("web-ui:///index.html");
        }
        Some(_) => {
            // NOTE: 127.0.0.1 needed to not accidentally use IPv6 localhost
            webview.load_uri("http:///127.0.0.1:1420/");
        }
    }

    webview.connect_decide_policy(decide_policy);

    webview.connect_load_changed(move |_webview, event| {
        if event == webkit6::LoadEvent::Started {
            let _ = webview_signals.unbounded_send(WebviewSignal::LoadStarted);
        }
    });

    webview.connect_web_process_terminated(|webview, reason| {
        tracing::error!(message_id = "Fq2xNr7V", ?reason, "webview process terminated, reloading");
        webview.reload();
    });

    // let inspector = webview.inspector().unwrap();
    // inspector.show();

    webview
}

// NOTE: We cannot forcibly set colorscheme in gtk
// gtk < 4.20 (Noble LTS has 4.18), has no way of forcibly setting light mode, only forcibly setting dark mode: https://docs.gtk.org/gtk4/property.Settings.gtk-application-prefer-dark-theme.html, which does work after init
// gtk >= 4.20 has this: https://docs.gtk.org/gtk4/property.CssProvider.prefers-color-scheme.html
// Attempts below:
//
// Attempt 1:
// let sc = window.style_context();
// let cssp = gtk::CssProvider::new();
// cssp.load_from_data(":root { color-scheme: light; }");
// gtk::style_context_add_provider_for_display(&display, &cssp, gtk::STYLE_PROVIDER_PRIORITY_USER);
//
// Attempt 2:
// let settings = gtk::Settings::default().unwrap();
// settings.set_gtk_application_prefer_dark_theme(true);
// let theme = settings.gtk_theme_name().unwrap();
// eprintln!("Theme: {theme:?}");
//
// let newtheme = theme.replace("dark", "light");
// settings.set_gtk_theme_name(Some("newtheme"));

fn decide_policy(_webview: &WebView, decision: &webkit6::PolicyDecision, decision_type: webkit6::PolicyDecisionType) -> bool {
    // SAFETY: Must check decision_type before casting decision: https://webkitgtk.org/reference/webkit2gtk/stable/enum.PolicyDecisionType.html
    let (webkit6::PolicyDecisionType::NavigationAction | webkit6::PolicyDecisionType::NewWindowAction) = decision_type else {
        eprintln!("CARL-NAV: not nav or window action: {:?}", decision_type);
        return false;
    };
    let nav_decision: &webkit6::NavigationPolicyDecision = decision.downcast_ref::<webkit6::NavigationPolicyDecision>().unwrap();

    let Some(mut nav_action) = nav_decision.navigation_action() else {
        eprintln!("CARL-NAV: no navigation action");
        return false;
    };

    let webkit6::NavigationType::LinkClicked = nav_action.navigation_type() else {
        eprintln!("CARL-NAV: Not link clicked");
        return false;
    };

    let Some(request) = nav_action.request() else {
        eprintln!("CARL-NAV: no request");
        return false;
    };
    eprintln!("CARL-NAV: request");
    eprintln!("CARL-NAV: uri: {:?}", request.uri());
    if let Some(headers) = request.http_headers() {
        headers.foreach(|k, v| {
            eprintln!("CARL-NAV: header: {k}={v}");
        });
    }
    eprintln!("CARL-NAV: method: {:?}", request.http_method());

    let Some(uri) = request.uri() else {
        eprintln!("CARL-NAV: no request URI");
        return false;
    };

    eprintln!("CARL-NAV: opening: {}", uri);

    open::that(uri.as_str()).unwrap();

    decision.ignore();

    true // https://webkitgtk.org/reference/webkit2gtk/stable/signal.WebView.decide-policy.html
}

// Ref: https://github.com/Sovereign-Engineering/obscuravpn-client-internal/blob/50ae1ec989463f1ff2a5b7ee12d11f58a1de5c1a/apple/client/command.swift#L9-L33
#[serde_with::serde_as]
#[derive(derive_more::Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Cmd {
    DebugBundle {
        user_feedback: String,
    },
    GetOsStatus {
        known_version: Option<Uuid>,
    },
    SetColorScheme {
        value: ColorScheme,
    },
    SetNavigationView {
        view: NavigationView,
    },
    JsonFfiCmd {
        cmd: String,
        timeout_ms: Option<serde_json::value::Number>,
    },
    StartTunnel {
        tunnel_args: String,
    },
    StopTunnel {},
    RevealItemInDir {
        path: String,
    },
    RestartService {
        enable: bool,
    },
    LinuxAddOperator {},
    RestartApp {},
    RegisterAsLoginItem {},
    UnregisterAsLoginItem {},
    RefreshLoginItemStatus {},
}

fn view_icon_name(view: NavigationView) -> &'static str {
    match view {
        NavigationView::Connection => "obscura-connection-symbolic",
        NavigationView::Location => "obscura-location-symbolic",
        NavigationView::Account => "obscura-account-symbolic",
        NavigationView::Settings => "obscura-settings-symbolic",
        NavigationView::Help => "obscura-help-symbolic",
        NavigationView::About => "obscura-about-symbolic",
        NavigationView::Developer => "obscura-developer-symbolic",
    }
}

pub(crate) enum WebviewSignal {
    CommandReceived,
    LoadStarted,
    PaymentSucceeded,
}

async fn dispatch_payment_succeeded(webview: &WebView) {
    let script = r#"window.dispatchEvent(new CustomEvent("paymentSucceeded"));"#;
    if let Err(error) = webview.evaluate_javascript_future(script, None, None).await {
        tracing::error!(message_id = "Hs5cYw7N", %error, "failed to dispatch paymentSucceeded event to webview");
    }
}

async fn forward_webview_events(webview: WebView, mut signals: UnboundedReceiver<WebviewSignal>) {
    let mut page_ready = false;
    let mut pending_payment_succeeded = false;
    while let Some(signal) = signals.next().await {
        match signal {
            WebviewSignal::CommandReceived => {
                if !page_ready {
                    page_ready = true;
                    if pending_payment_succeeded {
                        pending_payment_succeeded = false;
                        dispatch_payment_succeeded(&webview).await;
                    }
                }
            }
            WebviewSignal::LoadStarted => page_ready = false,
            WebviewSignal::PaymentSucceeded => {
                if page_ready {
                    dispatch_payment_succeeded(&webview).await;
                } else {
                    pending_payment_succeeded = true;
                }
            }
        }
    }
}

fn apply_color_scheme(color_scheme: ColorScheme) {
    StyleManager::default().set_color_scheme(match color_scheme {
        ColorScheme::Auto => libadwaita::ColorScheme::Default,
        ColorScheme::Light => libadwaita::ColorScheme::ForceLight,
        ColorScheme::Dark => libadwaita::ColorScheme::ForceDark,
    });
}

fn command_bridge(
    _ucm: &webkit6::UserContentManager,
    value: &webkit6::javascriptcore::Value,
    reply: &webkit6::ScriptMessageReply,
    gui_status: Arc<GuiStatusWatch>,
    restart: Rc<RefCell<Option<tokio::sync::oneshot::Sender<()>>>>,
    debug_bundler: Arc<GuiDebugBundler>,
    ui_config: Arc<UiConfigHandle>,
) -> bool {
    let command_json_gstring = value.to_str();
    let command_json_str = command_json_gstring.as_str();

    eprintln!("command_str: '{}'", command_json_str);

    // IMPORTANT: value context needs to live, just creating one doesn't do the proper lifetime
    // extension, especially when passed as rvalue like &context::new(), due to rust's temporary
    // lifetime extension rules
    let value_context = value.context().unwrap();

    let Ok(cmd): serde_json::Result<Cmd> = serde_json::from_str(command_json_str) else {
        eprintln!("CARL-UNKNOWN: '{}'", command_json_str);
        let error_msg = format!("Not implemented: '{}'", command_json_str);
        reply.return_error_message(&error_msg);

        return true;
    };
    eprintln!("deser: '{:?}'", cmd);

    match cmd {
        Cmd::DebugBundle { user_feedback } => {
            tokio_to_glib_local_fut_pipe(
                async move {
                    let result = debug_bundler.create(user_feedback).await.map_err(|error| match error {
                        DebugBundleError::InProgress => LinuxErrorCode::DebugBundleInProgress,
                        DebugBundleError::Failed => LinuxErrorCode::Other,
                    });
                    if let Ok(path) = &result {
                        let _ = reveal_item_in_dir(path.as_str()).await;
                    }
                    result
                },
                glib::clone!(
                    #[strong]
                    reply,
                    #[strong]
                    value_context,
                    move |res: Result<camino::Utf8PathBuf, LinuxErrorCode>| async move {
                        match res {
                            Ok(path) => {
                                let json_string = serde_json::to_string(&path).unwrap();
                                reply.return_value(&javascriptcore::Value::new_string(&value_context, Some(&json_string)));
                            }
                            Err(error) => reply.return_error_message(error.as_static_str()),
                        }
                    }
                ),
            );
        }
        Cmd::GetOsStatus { known_version } => {
            eprintln!("Got a call for GetOsStatus (non-FFI): '{:?}'", known_version);

            tokio_to_glib_local_fut_pipe(
                async move { gui_status.changed(known_version).await },
                glib::clone!(
                    #[strong]
                    reply,
                    #[strong]
                    value_context,
                    move |res| async move {
                        let json_string = serde_json::to_string(&res).unwrap();

                        // Our frontend actually expects a string repr of a json object  rather than an object
                        let jsc6_val = javascriptcore::Value::new_string(&value_context, Some(&json_string));

                        tracing::debug!(?json_string, "JsonFfiCmd: returning");
                        eprintln!("JsonFfiCmd: returning: '{:?}'", json_string);
                        reply.return_value(&jsc6_val.clone());
                        eprintln!("JsonFfiCmd: returned: '{:?}'", json_string);
                    }
                ),
            );
        }
        Cmd::SetColorScheme { value } => {
            apply_color_scheme(value);
            tokio_to_glib_local_fut_pipe(
                ui_config.update(move |ui_config| ui_config.color_scheme = value),
                glib::clone!(
                    #[strong]
                    reply,
                    #[strong]
                    value_context,
                    move |()| async move {
                        let json_string = serde_json::json!({}).to_string();
                        let jsc6_val = javascriptcore::Value::new_string(&value_context, Some(&json_string));
                        reply.return_value(&jsc6_val);
                    }
                ),
            );
        }
        Cmd::SetNavigationView { view } => {
            gui_status.set_navigation_view(view);
            let json_string = serde_json::json!({}).to_string();
            let jsc6_val = javascriptcore::Value::new_string(&value_context, Some(&json_string));
            reply.return_value(&jsc6_val);
        }
        Cmd::RestartService { enable } => {
            glib_run_linux_fix_and_reply(restart_service(enable), false, &value_context, reply, restart);
        }
        Cmd::LinuxAddOperator {} => {
            glib_run_linux_fix_and_reply(add_operator(), true, &value_context, reply, restart);
        }
        Cmd::RestartApp {} => {
            glib_run_linux_fix_and_reply(async { Ok(()) }, true, &value_context, reply, restart);
        }
        Cmd::RegisterAsLoginItem {} => {
            glib_run_login_item_cmd_and_reply(autostart::register_autostart(), &value_context, reply, gui_status);
        }
        Cmd::UnregisterAsLoginItem {} => {
            glib_run_login_item_cmd_and_reply(autostart::unregister_autostart(), &value_context, reply, gui_status);
        }
        Cmd::RefreshLoginItemStatus {} => {
            glib_run_login_item_cmd_and_reply(async { Ok(()) }, &value_context, reply, gui_status);
        }
        Cmd::JsonFfiCmd { ref cmd, timeout_ms } => {
            let mgr_cmd: ManagerCmd = serde_json::from_str(cmd).unwrap();

            glib_async_run_mgr_cmd_and_reply(mgr_cmd, &value_context, reply, timeout_ms);
        }
        Cmd::StartTunnel { tunnel_args } => {
            let args: TunnelArgs = serde_json::from_str(&tunnel_args).unwrap();
            let mgr_cmd: ManagerCmd = ManagerCmd::SetTunnelArgs { args: Some(args), active: Some(true) };

            glib_async_run_mgr_cmd_and_reply(mgr_cmd, &value_context, reply, None);
        }
        Cmd::StopTunnel {} => {
            let mgr_cmd: ManagerCmd = ManagerCmd::SetTunnelArgs { args: None, active: Some(false) };

            glib_async_run_mgr_cmd_and_reply(mgr_cmd, &value_context, reply, None);
        }
        Cmd::RevealItemInDir { path } => {
            tokio_to_glib_local_fut_pipe::<_, _, _, _>(
                glib::clone!(async move {
                    let _ = reveal_item_in_dir(&path).await;
                }),
                glib::clone!(
                    #[strong]
                    reply,
                    #[strong]
                    value_context,
                    move |()| async move {
                        let json_string = serde_json::json!({}).to_string();

                        // Our frontend actually expects a string repr of a json object  rather than an object
                        let jsc6_val = javascriptcore::Value::new_string(&value_context, Some(&json_string));

                        tracing::debug!(?json_string, "JsonFfiCmd: returning");
                        eprintln!("JsonFfiCmd: returning: '{:?}'", json_string);
                        reply.return_value(&jsc6_val.clone());
                        eprintln!("JsonFfiCmd: returned: '{:?}'", json_string);
                    }
                ),
            );
        }
    };

    true // https://webkitgtk.org/reference/webkit2gtk/stable/signal.UserContentManager.script-message-with-reply-received.html
}

// Pipe the output of a tokio future to a glib local future
fn tokio_to_glib_local_fut_pipe<TFO, TFut, GFut, GF>(tokio_fut: TFut, glib_fut: GF)
where
    TFut: Future<Output = TFO> + Send + 'static,
    GFut: Future<Output = ()>,
    GF: FnOnce(TFO) -> GFut + 'static,
    TFO: std::fmt::Debug + Send + 'static,
{
    let (sender, receiver) = futures::channel::oneshot::channel::<TFO>();

    glib::spawn_future_local(glib::clone!(async move {
        // IMPORTANT: needs to be async since this blocks the main thread
        let res: TFO = receiver.await.unwrap();
        glib_fut(res).await;
    }));

    tokio::spawn(glib::clone!(async move {
        let res: TFO = tokio_fut.await;
        sender.send(res).unwrap();
    }));
}

fn glib_run_linux_fix_and_reply(
    fix: impl Future<Output = Result<(), LinuxFixErrorCode>> + Send + 'static,
    restart_app: bool,
    value_context: &javascriptcore::Context,
    reply: &webkit6::ScriptMessageReply,
    restart: Rc<RefCell<Option<tokio::sync::oneshot::Sender<()>>>>,
) {
    tokio_to_glib_local_fut_pipe(
        async move {
            fix.await.map_err(LinuxErrorCode::from)?;
            Ok(restart_app)
        },
        glib::clone!(
            #[strong]
            reply,
            #[strong]
            value_context,
            move |res: Result<bool, LinuxErrorCode>| async move {
                match res {
                    Ok(restart_app) => {
                        reply.return_value(&javascriptcore::Value::new_string(
                            &value_context,
                            Some(&serde_json::json!({}).to_string()),
                        ));
                        if restart_app && let Some(quit) = restart.borrow_mut().take() {
                            let _ = quit.send(());
                        }
                    }
                    Err(error) => reply.return_error_message(error.as_static_str()),
                }
            }
        ),
    );
}

fn glib_run_login_item_cmd_and_reply(
    cmd: impl Future<Output = Result<(), ()>> + Send + 'static,
    value_context: &javascriptcore::Context,
    reply: &webkit6::ScriptMessageReply,
    gui_status: Arc<GuiStatusWatch>,
) {
    tokio_to_glib_local_fut_pipe(
        async move {
            let result = cmd.await;
            gui_status.refresh_login_item_status().await;
            result
        },
        glib::clone!(
            #[strong]
            reply,
            #[strong]
            value_context,
            move |res: Result<(), ()>| async move {
                match res {
                    Ok(()) => reply.return_value(&javascriptcore::Value::new_string(
                        &value_context,
                        Some(&serde_json::json!({}).to_string()),
                    )),
                    Err(()) => reply.return_error_message(LinuxErrorCode::Other.as_static_str()),
                }
            }
        ),
    );
}

fn glib_async_run_mgr_cmd_and_reply(
    mgr_cmd: ManagerCmd,
    value_context: &javascriptcore::Context,
    reply: &webkit6::ScriptMessageReply,
    timeout_ms: Option<serde_json::value::Number>,
) {
    eprintln!("Got a call for JsonFfiCmd: '{:?}'", mgr_cmd);

    tokio_to_glib_local_fut_pipe::<Result<serde_json::Value, LinuxErrorCode>, _, _, _>(
        async move {
            let mut last_error: Option<LinuxErrorCode> = None;
            for _attempt in 0..10 {
                let fut = run_command::<serde_json::Value>(mgr_cmd.clone());
                let run_command_res = if let Some(ref timeout_ms) = timeout_ms {
                    let Some(timeout_ms_u64) = timeout_ms.as_u64() else {
                        tracing::error!(message_id = "uW2fJn9K", %timeout_ms, "timeout_ms cannot be represented as u64");
                        return Err(LinuxErrorCode::Other);
                    };
                    match tokio::time::timeout(Duration::from_millis(timeout_ms_u64), fut).await {
                        Ok(res) => res,
                        Err(error) => {
                            tracing::error!(message_id = "oM5xDt3V", %error, timeout_ms = timeout_ms_u64, "manager command timed out");
                            last_error = Some(LinuxErrorCode::Other);
                            continue;
                        }
                    }
                } else {
                    fut.await
                };

                match run_command_res {
                    Ok(Ok(res)) => return Ok(res),
                    Ok(Err(error)) => {
                        tracing::error!(message_id = "gK8cQb4R", ?error, "manager command failed");
                        last_error = Some(error.into());
                    }
                    Err(error) => {
                        tracing::error!(message_id = "wZ3hSp7L", ?error, "failed to run manager command over IPC");
                        last_error = Some(LinuxErrorCode::from(&error));
                    }
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(last_error.unwrap())
        },
        glib::clone!(
            #[strong]
            reply,
            #[strong]
            value_context,
            move |res| async move {
                let res = match res {
                    Ok(res) => res,
                    Err(error) => {
                        reply.return_error_message(error.as_static_str());
                        return;
                    }
                };

                let json_string = res.to_string();

                // Our frontend actually expects a string repr of a json object  rather than an object
                let jsc6_val = javascriptcore::Value::new_string(&value_context, Some(&json_string));

                tracing::debug!(?json_string, "JsonFfiCmd: returning");
                eprintln!("JsonFfiCmd: returning: '{:?}'", json_string);
                reply.return_value(&jsc6_val.clone());
                eprintln!("JsonFfiCmd: returned: '{:?}'", json_string);
            }
        ),
    );
}

fn error_handler(_ucm: &webkit6::UserContentManager, value: &webkit6::javascriptcore::Value) {
    eprintln!("error str: '{}'", value.to_str());
}

fn log_handler(_ucm: &webkit6::UserContentManager, value: &webkit6::javascriptcore::Value) {
    eprintln!("log str: '{}'", value.to_str());
}

fn view_row_widget(view: NavigationView) -> gtk::Box {
    let hbox = gtk::Box::new(Orientation::Horizontal, 8);
    let icon = gtk::Image::from_icon_name(view_icon_name(view));
    icon.set_pixel_size(24);
    let label = Label::builder()
        .halign(Align::Start)
        .valign(Align::Center)
        .label(view.to_string())
        .build();
    hbox.append(&icon);
    hbox.append(&label);
    hbox
}

fn view_at_row_index(index: i32) -> Option<NavigationView> {
    let index = usize::try_from(index).ok()?;
    NavigationView::iter().nth(index)
}

fn sidebar(gui_status: Arc<GuiStatusWatch>, dev_visible: Rc<Cell<bool>>) -> ListBox {
    let list = ListBox::builder()
        .selection_mode(SelectionMode::Browse)
        .css_classes(vec!["navigation-sidebar".to_owned(), "sidebar".to_owned()])
        .build();

    for view in NavigationView::iter() {
        list.append(&view_row_widget(view));
    }

    list.set_filter_func(move |row| match view_at_row_index(row.index()) {
        Some(NavigationView::Developer) => dev_visible.get(),
        Some(_) | None => true,
    });

    list.connect_row_selected(move |lb, mb_lbr| {
        // Try to select first row if none selected
        let Some(lbr) = mb_lbr else {
            let Some(first_row) = lb.row_at_index(0) else {
                return;
            };
            lb.select_row(Some(&first_row));
            return;
        };

        let Some(view) = view_at_row_index(lbr.index()) else {
            return;
        };
        gui_status.set_navigation_view(view);
    });

    list
}

fn build_primary_window(
    gui_status: Arc<GuiStatusWatch>,
    restart: Rc<RefCell<Option<tokio::sync::oneshot::Sender<()>>>>,
    debug_bundler: Arc<GuiDebugBundler>,
    webview_signals: UnboundedSender<WebviewSignal>,
    ui_config: Arc<UiConfigHandle>,
) -> (gtk::ApplicationWindow, ListBox, WebView) {
    let window = gtk::ApplicationWindow::builder()
        .title("Obscura VPN")
        .hide_on_close(true) // So that closing window doesn't quit app
        .default_width(800)
        .default_height(600)
        .build();

    let display = gdk::Display::default().expect("Could not get default display");
    let icon_theme = gtk::IconTheme::for_display(&display);
    icon_theme.add_resource_path("/com/obscura/vpn/icons/icons");

    let dev_visible = Rc::new(Cell::new(false));

    let (split_view, sidebar, webview) = navigation_split_view(gui_status, restart, dev_visible.clone(), debug_bundler, webview_signals, ui_config);
    window.set_child(Some(&split_view));

    // Ctrl+Shift+D toggles Developer sidebar item
    let controller = gtk::ShortcutController::new();
    controller.set_scope(gtk::ShortcutScope::Local);
    let shortcut = gtk::Shortcut::new(
        gtk::ShortcutTrigger::parse_string("<Control><Shift>d"),
        Some(gtk::CallbackAction::new(glib::clone!(
            #[strong]
            dev_visible,
            #[strong]
            sidebar,
            move |_widget, _args| {
                dev_visible.update(std::ops::Not::not);
                sidebar.invalidate_filter();
                glib::Propagation::Stop
            }
        ))),
    );
    controller.add_shortcut(shortcut);
    window.add_controller(controller);

    (window, sidebar, webview)
}

fn select_view_row(sidebar: &ListBox, view: NavigationView) {
    let Some(index) = NavigationView::iter().position(|v| v == view) else {
        return;
    };
    let Ok(index) = i32::try_from(index) else {
        return;
    };
    let Some(row) = sidebar.row_at_index(index) else {
        tracing::warn!(message_id = "Cp7gV3ol", view = %view, "no sidebar row for view");
        return;
    };
    sidebar.select_row(Some(&row));
}

fn open_app_url(uri: &str, window: &gtk::ApplicationWindow, gui_status: &GuiStatusWatch, webview_signals: &UnboundedSender<WebviewSignal>) {
    tracing::info!(message_id = "kW2sQp9J", %uri, "opening app URL");
    window.present();
    let parsed = match glib::Uri::parse(uri, glib::UriFlags::NONE) {
        Ok(parsed) => parsed,
        Err(error) => {
            tracing::error!(message_id = "Vb6nTq3X", %uri, %error, "failed to parse app URL");
            return;
        }
    };
    if parsed.scheme() != "obscuravpn" {
        tracing::error!(message_id = "Rj8mFd4Z", %uri, "ignoring app URL with unexpected scheme");
        return;
    }
    match parsed.path().as_str() {
        "" | "/" | "/open" => {}
        "/account" | "/manage-subscription" => gui_status.set_navigation_view(NavigationView::Account),
        "/location" => gui_status.set_navigation_view(NavigationView::Location),
        "/payment-succeeded" => {
            let _ = webview_signals.unbounded_send(WebviewSignal::PaymentSucceeded);
        }
        path => tracing::error!(message_id = "Gt4kLp2M", %path, "unknown app URL path"),
    }
}

fn print_gresources(res: &gio::Resource, path: &str) {
    match res.enumerate_children(path, gio::ResourceLookupFlags::NONE) {
        Ok(children) => {
            for child in children {
                let child_path = format!("{}{}", path, child);
                if child.ends_with('/') {
                    print_gresources(res, &child_path);
                } else {
                    eprintln!("  resource: {}", child_path);
                }
            }
        }
        Err(e) => eprintln!("  error enumerating {}: {}", path, e),
    }
}

/// Must run on the main thread. Blocks until the app quits.
pub(crate) fn run_gtk_app(
    _main_thread: MainThreadToken,
    gui_status: Arc<GuiStatusWatch>,
    mut tray_receiver: Receiver<TrayRequest>,
    debug_bundler: Arc<GuiDebugBundler>,
    urls: Vec<String>,
    ui_config: Arc<UiConfigHandle>,
) -> GtkAppFinished {
    eprintln!("First light");

    // So that we can initialize our window without being in connect_activate/startup Fn scope
    // (which is not FnOnce), see: https://gtk-rs.org/gtk4-rs/stable/latest/docs/gtk4/fn.init.html
    let Ok(()) = gtk::init() else {
        eprintln!("Failed to init gtk4");
        return GtkAppFinished::Exit(ExitCode::FAILURE);
    };
    let Ok(()) = libadwaita::init() else {
        eprintln!("Failed to init libadwaita");
        return GtkAppFinished::Exit(ExitCode::FAILURE);
    };
    apply_color_scheme(ui_config.get().color_scheme);

    let resources_bytes: &[u8] = include_bytes!(concat!(env!("OBSCURA_GRESOURCES_DIR"), "/icons.gresource"));
    let gbytes = glib::Bytes::from_static(resources_bytes);
    let res = gio::Resource::from_data(&gbytes).expect("Could not load gresource file");
    gio::resources_register(&res);

    eprintln!("Registered gresources:");
    print_gresources(&res, "/");

    let resources_bytes2: &[u8] = include_bytes!(concat!(env!("OBSCURA_GRESOURCES_DIR"), "/webui.gresource"));
    let gbytes2 = glib::Bytes::from_static(resources_bytes2);
    let res2 = gio::Resource::from_data(&gbytes2).expect("Could not load gresource file");
    gio::resources_register(&res2);

    eprintln!("Registered gresources:");
    print_gresources(&res2, "/");

    let app = gtk::Application::builder()
        .application_id("net.obscura.vpn.gui")
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    let restart_requested = Arc::new(AtomicBool::new(false));
    let (quit_tx, quit_rx) = tokio::sync::oneshot::channel();
    let restart = Rc::new(RefCell::new(Some(quit_tx)));
    let (webview_signals, webview_signal_receiver) = unbounded();
    let (window, sidebar, webview) = build_primary_window(gui_status.clone(), restart.clone(), debug_bundler, webview_signals.clone(), ui_config);

    glib::spawn_future_local(forward_webview_events(webview, webview_signal_receiver));

    glib::spawn_future_local(glib::clone!(
        #[strong]
        sidebar,
        #[strong]
        gui_status,
        async move {
            let mut known_version = None;
            loop {
                let status = gui_status.changed(known_version).await;
                known_version = Some(status.version);
                select_view_row(&sidebar, status.navigation_view);
            }
        }
    ));

    glib::spawn_future_local(glib::clone!(
        #[strong]
        app,
        #[strong]
        window,
        #[strong]
        gui_status,
        async move {
            while let Some(request) = tray_receiver.next().await {
                match request {
                    TrayRequest::Show(target) => {
                        window.present();
                        match target {
                            ShowTarget::MainWindow => {}
                            ShowTarget::LocationView => gui_status.set_navigation_view(NavigationView::Location),
                        }
                    }
                    TrayRequest::Quit => app.quit(),
                }
            }
        }
    ));

    app.connect_startup(glib::clone!(
        #[strong]
        window,
        #[strong]
        gui_status,
        move |app| {
            app.add_window(&window);
            tokio::spawn(auto_connect::auto_connect_if_enabled(gui_status.clone()));
        }
    ));

    app.connect_activate(glib::clone!(
        #[strong]
        window,
        move |_app| {
            window.present();
        }
    ));

    app.connect_open(glib::clone!(
        #[strong]
        window,
        #[strong]
        gui_status,
        #[strong]
        webview_signals,
        move |_app, files, _hint| {
            for file in files {
                open_app_url(&file.uri(), &window, &gui_status, &webview_signals);
            }
        }
    ));

    let restart_requested_setter = restart_requested.clone();
    tokio_to_glib_local_fut_pipe(
        async move {
            tokio::select! {
                result = tokio::signal::ctrl_c() => result.expect("failed to listen for ctrl-c"),
                result = quit_rx => {
                    if result.is_ok() {
                        restart_requested_setter.store(true, Ordering::Relaxed);
                    }
                }
            }
        },
        glib::clone!(
            #[strong]
            app,
            move |()| async move {
                eprintln!("CARL: quitting gracefully");
                app.quit();
            }
        ),
    );

    let mut gtk_args = vec!["obscura-gui".to_string()];
    gtk_args.extend(urls);
    let gtk_exit_code = app.run_with_args(&gtk_args);
    if restart_requested.load(Ordering::Relaxed) {
        GtkAppFinished::Restart
    } else {
        GtkAppFinished::Exit(u8::try_from(gtk_exit_code.value()).map(ExitCode::from).unwrap_or(ExitCode::FAILURE))
    }
}

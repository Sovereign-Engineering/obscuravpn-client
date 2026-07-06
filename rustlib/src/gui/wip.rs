// ============================================================================
// TODO: REFACTOR EVERYTHING IN HERE AND REMOVE THIS FILE
// ============================================================================
#![allow(unused_imports)]
#![allow(clippy::enum_variant_names)]

use futures::StreamExt;
use gtk4::gio::{DBusError, DBusProxy, ResourceLookupFlags};
use gtk4::glib::translate::ToGlibPtr as _;
use ksni::TrayMethods;
use obscuravpn_client::debug_bundle::bundle_info::BundleInfo;
use obscuravpn_client::exit_selection::ExitSelector;
use obscuravpn_client::linux::{ClientError, run_command};
use obscuravpn_client::manager::{self, TunnelArgs};
use obscuravpn_client::manager_cmd::{ManagerCmd, ManagerCmdErrorCode, ManagerCmdOk};
use serde::{Deserialize, Serialize};
use std::cell::{Cell, RefCell};
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::{Arc, OnceLock, mpsc};
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
use zbus_polkit::policykit1::{CheckAuthorizationFlags, Subject};
use zbus_systemd::zbus; // provides the spawn method

#[derive(Debug)]
struct MyTray {
    selected_option: usize,
    checked: bool,
    os_status: Option<OsStatusFull>,
    open_sender: futures::channel::mpsc::UnboundedSender<()>,
}

impl ksni::Tray for MyTray {
    fn id(&self) -> String {
        env!("CARGO_PKG_NAME").into()
    }

    fn status(&self) -> ksni::Status {
        let Some(OsStatusFull { rest: OsStatus { os_vpn_status: ref osvpns, .. }, .. }) = self.os_status else {
            return ksni::Status::NeedsAttention;
        };

        // tray icon is hidden if not active
        match osvpns {
            NEVPNStatus::Invalid => ksni::Status::NeedsAttention,
            _ => ksni::Status::Active,
        }
    }

    fn icon_name(&self) -> String {
        let Some(OsStatusFull { rest: OsStatus { os_vpn_status: ref osvpns, .. }, .. }) = self.os_status else {
            return "network-error".to_owned();
        };

        match osvpns {
            NEVPNStatus::Invalid => "network-error".to_owned(),
            NEVPNStatus::Disconnected => "network-offline".to_owned(),
            NEVPNStatus::Connecting => "network-offline".to_owned(),
            NEVPNStatus::Connected => "network-wired".to_owned(),
            NEVPNStatus::Reasserting => "network-offline".to_owned(),
            NEVPNStatus::Disconnecting => "network-offline".to_owned(),
        }
    }

    fn title(&self) -> String {
        if self.checked { "CHECKED!" } else { "MyTray" }.into()
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "Connect".to_owned(),
                activate: Box::new(|_| {
                    tray_connect();
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Disconnect".to_owned(),
                activate: Box::new(|_| {
                    tray_disconnect();
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Open Obscura Manager...".to_owned(),
                activate: Box::new(|this: &mut Self| {
                    this.open_sender.unbounded_send(()).unwrap();
                }),
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            SubMenu {
                label: "a".into(),
                submenu: vec![
                    SubMenu {
                        label: "a1".into(),
                        submenu: vec![
                            StandardItem { label: "a1.1".into(), ..Default::default() }.into(),
                            StandardItem { label: "a1.2".into(), ..Default::default() }.into(),
                        ],
                        ..Default::default()
                    }
                    .into(),
                    StandardItem { label: "a2".into(), ..Default::default() }.into(),
                ],
                ..Default::default()
            }
            .into(),
            MenuItem::Separator,
            RadioGroup {
                selected: self.selected_option,
                select: Box::new(|this: &mut Self, current| {
                    this.selected_option = current;
                }),
                options: vec![
                    RadioItem { label: "Option 0".into(), ..Default::default() },
                    RadioItem { label: "Option 1".into(), ..Default::default() },
                    RadioItem { label: "Option 2".into(), ..Default::default() },
                ],
            }
            .into(),
            CheckmarkItem {
                label: "Checkable".into(),
                checked: self.checked,
                activate: Box::new(|this: &mut Self| this.checked = !this.checked),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Exit".into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|_| std::process::exit(0)),
                ..Default::default()
            }
            .into(),
        ]
    }
}

fn navigation_split_view(osskeeper: Arc<OsStatusKeeper>, dev_visible: Rc<Cell<bool>>) -> (gtk::Box, ListBox) {
    let split_view = gtk::Box::new(Orientation::Horizontal, 0);

    let webview = webview(osskeeper);
    webview.set_hexpand(true);

    let sidebar = sidebar(&webview, dev_visible);

    split_view.append(&sidebar);
    split_view.append(&webview);

    (split_view, sidebar)
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

fn webview(osskeeper: Arc<OsStatusKeeper>) -> WebView {
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

    user_content_manager.connect_script_message_with_reply_received(Some("commandBridge"), move |ucm, value, reply| {
        command_bridge(ucm, value, reply, osskeeper.clone())
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
    GetOsStatus {
        known_version: Option<Uuid>,
    },
    JsonFfiCmd {
        cmd: String,
        timeout_ms: Option<serde_json::value::Number>,
    },
    StartTunnel {
        tunnel_args: String,
    },
    StopTunnel {},
    DebuggingArchive {
        user_feedback: Option<String>,
    },
    RevealItemInDir {
        path: String,
    },
    LinuxFix {
        action: LinuxFixAction,
    },
}

#[serde_with::serde_as]
#[derive(derive_more::Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum NEVPNStatus {
    Invalid,
    Disconnected,
    Connecting,
    Connected,
    Reasserting,
    Disconnecting,
}

impl From<manager::VpnStatus> for NEVPNStatus {
    fn from(value: manager::VpnStatus) -> Self {
        match value {
            manager::VpnStatus::Connecting { .. } => Self::Connecting,
            manager::VpnStatus::Connected { .. } => Self::Connected,
            manager::VpnStatus::Disconnected { .. } => Self::Disconnected,
        }
    }
}

#[serde_with::serde_as]
#[derive(derive_more::Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OsStatus {
    internet_available: bool,
    os_vpn_status: NEVPNStatus,
    src_version: String,
    updater_status: UpdaterStatus,
    debug_bundle_status: DebugBundleStatus,
    can_send_mail: bool,
    service_status: ServiceStatus,
}

#[derive(derive_more::Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ServiceStatus {
    Healthy,
    Degraded(Degradation),
}

#[derive(derive_more::Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Degradation {
    LinuxDegraded(LinuxDegradation),
    OtherDegraded,
}

#[derive(derive_more::Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(rename_all = "camelCase")]
pub enum LinuxDegradation {
    Stopped,
    Failed,
    Disabled,
    NotInstalled,
    NoAccess,
}

#[derive(derive_more::Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum LinuxFixAction {
    Start,
    EnableAndStart,
    AddOperator,
}

impl OsStatus {
    fn to_hash(&self) -> Uuid {
        let mut hasher = std::hash::DefaultHasher::new();
        self.hash(&mut hasher);
        let hash = hasher.finish();

        Uuid::from_u64_pair(hash, hash)
    }
}

impl From<OsStatus> for OsStatusFull {
    fn from(value: OsStatus) -> Self {
        let uuid = value.to_hash();

        OsStatusFull { version: uuid, rest: value }
    }
}

#[serde_with::serde_as]
#[derive(derive_more::Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OsStatusFull {
    version: Uuid,
    #[serde(flatten)]
    rest: OsStatus,
}

impl Default for OsStatusFull {
    fn default() -> Self {
        OsStatus::default().into()
    }
}

#[derive(Default)]
pub struct OsStatusKeeper {
    inner: tokio::sync::watch::Sender<OsStatusFull>,
}

impl OsStatusKeeper {
    async fn wait_next_status_inner(&self, known_version: Option<Uuid>) -> OsStatusFull {
        let mut rx = self.inner.subscribe();
        let new = rx.wait_for(|oss| Some(oss.version) != known_version).await.unwrap();
        (*new).clone()
    }

    async fn wait_next_status(&self, known_version: Option<Uuid>) -> OsStatusFull {
        let mut manager_version: Option<Uuid> = None;
        loop {
            tokio::select! {
                reply = self.wait_next_status_inner(known_version) => {
                    return reply
                }
                maybe_res = run_command::<manager::Status>(ManagerCmd::GetStatus { known_version: manager_version }) => {
                    let mut backoff = false;
                    let (service_status, os_vpn_status) = match maybe_res {
                        Ok(Ok(res)) => {
                            manager_version = Some(res.version);
                            (ServiceStatus::Healthy, NEVPNStatus::from(res.vpn_status))
                        }
                        Err(ClientError::NoService) => {
                            backoff = true;
                            (ServiceStatus::Degraded(Degradation::LinuxDegraded(diagnose_linux_service().await)), NEVPNStatus::Invalid)
                        }
                        Err(ClientError::InsufficientPermissions) => {
                            backoff = true;
                            (ServiceStatus::Degraded(Degradation::LinuxDegraded(LinuxDegradation::NoAccess)), NEVPNStatus::Invalid)
                        }
                        Ok(Err(error)) => {
                            eprintln!("status command error: {err}", err = ClientError::from(error));
                            backoff = true;
                            (ServiceStatus::Degraded(Degradation::OtherDegraded), NEVPNStatus::Invalid)
                        }
                        Err(err) => {
                            eprintln!("failed to get status: {err}");
                            backoff = true;
                            (ServiceStatus::Degraded(Degradation::OtherDegraded), NEVPNStatus::Invalid)
                        }
                    };

                    self.send_if_modified(|status| -> bool {
                        let changed = status.service_status != service_status || status.os_vpn_status != os_vpn_status;
                        status.service_status = service_status;
                        status.os_vpn_status = os_vpn_status;
                        changed
                    });

                    if backoff {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        }
    }

    pub fn send_if_modified<F>(&self, modify: F) -> bool
    where
        F: FnOnce(&mut OsStatus) -> bool,
    {
        self.inner.send_if_modified(|status: &mut OsStatusFull| -> bool {
            let was_modified = modify(&mut status.rest);
            if was_modified {
                status.version = status.rest.to_hash();
            }
            was_modified
        })
    }
}

impl Default for OsStatus {
    fn default() -> Self {
        Self {
            internet_available: true,
            os_vpn_status: NEVPNStatus::Invalid,
            src_version: option_env!("OBSCURA_VERSION").unwrap_or("v0.0.0-dev").to_owned(),
            updater_status: Default::default(),
            debug_bundle_status: Default::default(),
            can_send_mail: true, // FIXME
            service_status: ServiceStatus::Healthy,
        }
    }
}

#[serde_with::serde_as]
#[derive(derive_more::Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppcastSummary {
    pub date: String,
    pub description: String,
    pub version: String,
    pub min_system_version_sdk: bool,
}

#[serde_with::serde_as]
#[derive(derive_more::Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UpdaterStatus {
    pub r#type: UpdaterStatusType,
    pub appcast: Option<AppcastSummary>,
    pub error: Option<String>,
    pub error_code: Option<i32>,
}

#[serde_with::serde_as]
#[derive(Default, derive_more::Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(rename_all = "camelCase")]
pub enum UpdaterStatusType {
    #[default]
    Uninitiated,
    Initiated,
    Available,
    NotFound,
    Error,
}

#[allow(clippy::derivable_impls)]
impl Default for UpdaterStatus {
    fn default() -> Self {
        Self { r#type: Default::default(), appcast: Default::default(), error: None, error_code: None }
    }
}

#[serde_with::serde_as]
#[derive(derive_more::Debug, Serialize, Deserialize, PartialEq, Eq, Hash, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DebugBundleStatus {
    pub in_progress: bool,
    pub latest_path: Option<String>,
    pub in_progress_counter: i64,
}

#[allow(clippy::derivable_impls)]
impl Default for DebugBundleStatus {
    fn default() -> Self {
        Self { in_progress: false, latest_path: None, in_progress_counter: 0 }
    }
}

#[derive(strum::EnumIter, strum::Display, strum::EnumString, Default, PartialEq)]
pub enum AppView {
    #[default]
    Connection,
    Location,
    Account,
    Settings,
    Help,
    About,
    Developer,
}

impl AppView {
    // Used for navigation
    fn ipc_value(&self) -> String {
        self.to_string().to_lowercase()
    }

    fn icon_name(&self) -> &'static str {
        match self {
            AppView::Connection => "obscura-connection-symbolic",
            AppView::Location => "obscura-location-symbolic",
            AppView::Account => "obscura-account-symbolic",
            AppView::Settings => "obscura-settings-symbolic",
            AppView::Help => "obscura-help-symbolic",
            AppView::About => "obscura-about-symbolic",
            AppView::Developer => "obscura-developer-symbolic",
        }
    }
}

async fn navigate_webview_to(webview: &WebView, av: &AppView) {
    let script = navigate_js(av);
    if let Err(error) = webview.evaluate_javascript_future(&script, None, None).await {
        tracing::warn!(message_id = "JdNqtceL", view = %av, %error, "failed to dispatch navigation event to webview");
    }
}

fn navigate_js(av: &AppView) -> String {
    format!(
        r###"__WEBKIT_NAV_EVENT__ = new CustomEvent("navUpdate", {{ detail: "{ipc_value}" }});
window.dispatchEvent(__WEBKIT_NAV_EVENT__);
"###,
        ipc_value = av.ipc_value()
    )
    .to_string()
}

fn command_bridge(
    _ucm: &webkit6::UserContentManager,
    value: &webkit6::javascriptcore::Value,
    reply: &webkit6::ScriptMessageReply,
    osskeeper: Arc<OsStatusKeeper>,
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
        Cmd::GetOsStatus { known_version } => {
            eprintln!("Got a call for GetOsStatus (non-FFI): '{:?}'", known_version);

            tokio_to_glib_local_fut_pipe(
                async move { osskeeper.wait_next_status(known_version).await },
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
        Cmd::LinuxFix { action } => {
            tokio_to_glib_local_fut_pipe(
                async move { apply_linux_fix(action).await },
                glib::clone!(
                    #[strong]
                    reply,
                    #[strong]
                    value_context,
                    move |res: Result<(), String>| async move {
                        match res {
                            Ok(()) => reply.return_value(&javascriptcore::Value::new_string(&value_context, None)),
                            Err(error) => reply.return_error_message(&error),
                        }
                    }
                ),
            );
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
        Cmd::DebuggingArchive { user_feedback } => {
            let mgr_cmd: ManagerCmd = ManagerCmd::CreateDebugBundle {
                user_feedback,
                bundle_info: BundleInfo { app_version: OsStatus::default().src_version, ..Default::default() },
            };

            tokio_to_glib_local_fut_pipe::<Result<String, String>, _, _, _>(
                glib::clone!(
                    #[strong]
                    osskeeper,
                    async move {
                        osskeeper.send_if_modified(|status| {
                            status.debug_bundle_status.in_progress = true;
                            true
                        });

                        let fut = run_command::<String>(mgr_cmd);
                        let run_command_res = fut.await;

                        let res = run_command_res
                            .map_err(|e| ToString::to_string(&e))?
                            .map_err(|e| format!("manager command error: {e:?}").to_owned())?;

                        Ok(res)
                    }
                ),
                glib::clone!(
                    #[strong]
                    reply,
                    #[strong]
                    value_context,
                    #[strong]
                    osskeeper,
                    move |res| async move {
                        let path = match res {
                            Ok(res) => res,
                            Err(error) => {
                                osskeeper.send_if_modified(|status| {
                                    status.debug_bundle_status.in_progress = false;
                                    true
                                });
                                reply.return_error_message(&error);
                                return;
                            }
                        };
                        osskeeper.send_if_modified(|status| {
                            status.debug_bundle_status.in_progress = false;
                            status.debug_bundle_status.latest_path = Some(path.clone());
                            true
                        });

                        let json_string = serde_json::to_string(&path.clone()).unwrap();

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
        Cmd::RevealItemInDir { path } => {
            tokio_to_glib_local_fut_pipe::<_, _, _, _>(
                glib::clone!(async move {
                    show_file2(&path).await;
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

async fn show_file2(path: &str) {
    let url = url::Url::from_file_path(path).unwrap();

    //https://www.freedesktop.org/wiki/Specifications/file-manager-interface/?__goaway_challenge=meta-refresh&__goaway_id=898e1d2637d83c80b5de59a2eb5555f3&__goaway_referer=https%3A%2F%2Fdocs.rs%2F
    #[zbus::proxy(
        interface = "org.freedesktop.FileManager1",
        default_service = "org.freedesktop.FileManager1",
        default_path = "/org/freedesktop/FileManager1"
    )]
    trait FileManager1 {
        async fn show_items(&self, uris: Vec<&str>, startup_id: &str) -> zbus::Result<()>;
    }

    let conn = zbus::Connection::session().await.unwrap();
    let proxy = FileManager1Proxy::new(&conn).await.unwrap();
    proxy.show_items(vec![url.as_ref()], "").await.unwrap();
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

    tokio_rt().spawn(glib::clone!(async move {
        let res: TFO = tokio_fut.await;
        sender.send(res).unwrap();
    }));
}

fn glib_async_run_mgr_cmd_and_reply(
    mgr_cmd: ManagerCmd,
    value_context: &javascriptcore::Context,
    reply: &webkit6::ScriptMessageReply,
    timeout_ms: Option<serde_json::value::Number>,
) {
    eprintln!("Got a call for JsonFfiCmd: '{:?}'", mgr_cmd);

    tokio_to_glib_local_fut_pipe::<Result<serde_json::Value, String>, _, _, _>(
        async move {
            let mut last_error: Option<String> = None;
            for _attempt in 0..10 {
                let fut = run_command::<serde_json::Value>(mgr_cmd.clone());
                let run_command_res = if let Some(ref timeout_ms) = timeout_ms {
                    let Some(timeout_ms_u64) = timeout_ms.as_u64() else {
                        return Err(format!("timeout_ms cannot be represented as u64: '{timeout_ms}'").to_owned());
                    };
                    match tokio::time::timeout(Duration::from_millis(timeout_ms_u64), fut).await {
                        Ok(res) => res,
                        Err(err) => {
                            last_error = Some(err.to_string());
                            continue;
                        }
                    }
                } else {
                    fut.await
                };

                match run_command_res {
                    Ok(Ok(res)) => return Ok(res),
                    Ok(Err(error)) => {
                        let err = ClientError::from(error);
                        eprintln!("Failed to connect: {err}");
                        last_error = Some(err.to_string());
                    }
                    Err(err) => {
                        eprintln!("Failed to connect: {err}");
                        last_error = Some(err.to_string());
                    }
                }

                tokio::time::sleep(Duration::from_secs(1)).await;
            }
            Err(format!("max attempt passed: {last_error}", last_error = last_error.unwrap()))
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
                        reply.return_error_message(&error);
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

fn appview_to_row_widget(appview: &AppView) -> gtk::Box {
    let hbox = gtk::Box::new(Orientation::Horizontal, 8);
    let icon = gtk::Image::from_icon_name(appview.icon_name());
    icon.set_pixel_size(24);
    let label = Label::builder()
        .halign(Align::Start)
        .valign(Align::Center)
        .label(appview.to_string())
        .build();
    hbox.append(&icon);
    hbox.append(&label);
    hbox
}

fn appview_string_to_row_widget(av_str: &str) -> gtk::Box {
    let av = AppView::from_str(av_str).unwrap();
    appview_to_row_widget(&av)
}

fn sidebar(webview: &WebView, dev_visible: Rc<Cell<bool>>) -> ListBox {
    // NOTE: Using StringList as the model may be overkill or not
    let model = {
        let rust_model_owned: Vec<String> = AppView::iter().map(|av| av.to_string()).collect();

        let rust_model_2: Vec<&str> = rust_model_owned.iter().map(String::as_str).collect();

        let rust_model: &[&str] = rust_model_2.as_slice();

        gtk::StringList::new(rust_model)
    };

    let list = ListBox::builder()
        .selection_mode(SelectionMode::Browse)
        .css_classes(vec!["navigation-sidebar".to_owned(), "sidebar".to_owned()])
        .build();

    list.bind_model(Some(&model), move |obj| {
        let list_object: String = obj
            .downcast_ref::<gtk::StringObject>()
            .expect("The object should be of type `StringObject`.")
            .into();
        appview_string_to_row_widget(&list_object).upcast()
    });

    list.set_filter_func(glib::clone!(
        #[strong]
        model,
        move |row| {
            let row_index: u32 = row.index().try_into().unwrap();
            let av_gstring: gtk::StringObject = model.item(row_index).and_downcast::<gtk::StringObject>().unwrap();
            let av_string = String::from(av_gstring);
            if av_string == "Developer" {
                return dev_visible.get();
            }
            true
        }
    ));

    list.connect_row_selected(glib::clone!(
        #[strong]
        webview,
        move |lb, mb_lbr| {
            // Try to select first row if none selected
            let Some(lbr) = mb_lbr else {
                let Some(first_row) = lb.row_at_index(0) else {
                    return;
                };
                lb.select_row(Some(&first_row));
                return;
            };

            glib::spawn_future_local(glib::clone!(
                #[strong]
                lbr,
                #[strong]
                model,
                #[strong]
                webview,
                async move {
                    let av = lbr_to_appview(&lbr, &model);

                    navigate_webview_to(&webview, &av).await;
                }
            ));
        }
    ));

    list
}

fn lbr_to_appview(lbr: &gtk::ListBoxRow, model: &impl IsA<gio::ListModel>) -> AppView {
    let row_index: u32 = lbr.index().try_into().unwrap();

    let av_gstring: gtk::StringObject = model.item(row_index).and_downcast::<gtk::StringObject>().unwrap();

    let av_string = String::from(av_gstring);

    AppView::from_str(&av_string).unwrap()
}

fn tokio_rt() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().expect("Setting up tokio runtime needs to succeed."))
}

fn tray_connect() {
    let args = TunnelArgs { exit: ExitSelector::Any {} };
    let mgr_cmd: ManagerCmd = ManagerCmd::SetTunnelArgs { args: Some(args), active: Some(true) };
    tokio_rt().spawn(async {
        let fut = run_command::<serde_json::Value>(mgr_cmd);
        match fut.await {
            Ok(Ok(..)) => {}
            Ok(Err(error)) => eprintln!("Failed to connect: {err}", err = ClientError::from(error)),
            Err(err) => eprintln!("Failed to connect: {err}"),
        }
    });
}

fn tray_disconnect() {
    let mgr_cmd: ManagerCmd = ManagerCmd::SetTunnelArgs { args: None, active: Some(false) };

    tokio_rt().spawn(async {
        let fut = run_command::<serde_json::Value>(mgr_cmd);
        match fut.await {
            Ok(Ok(..)) => {}
            Ok(Err(error)) => eprintln!("Failed to disconnect: {err}", err = ClientError::from(error)),
            Err(err) => eprintln!("Failed to disconnect: {err}"),
        }
    });
}

fn build_primary_window(osskeeper: Arc<OsStatusKeeper>) -> gtk::ApplicationWindow {
    let window = gtk::ApplicationWindow::builder()
        .hide_on_close(true) // So that closing window doesn't quit app
        .default_width(800)
        .default_height(600)
        .build();

    let display = gdk::Display::default().expect("Could not get default display");
    let icon_theme = gtk::IconTheme::for_display(&display);
    icon_theme.add_resource_path("/com/obscura/vpn/icons/icons");

    let dev_visible = Rc::new(Cell::new(false));

    let (split_view, sidebar) = navigation_split_view(osskeeper, dev_visible.clone());
    window.set_child(Some(&split_view));

    // Ctrl+Shift+D toggles Developer sidebar item
    let controller = gtk::ShortcutController::new();
    controller.set_scope(gtk::ShortcutScope::Local);
    let shortcut = gtk::Shortcut::new(
        gtk::ShortcutTrigger::parse_string("<Control><Shift>d"),
        Some(gtk::CallbackAction::new(glib::clone!(
            #[strong]
            dev_visible,
            move |_widget, _args| {
                dev_visible.update(std::ops::Not::not);
                sidebar.invalidate_filter();
                glib::Propagation::Stop
            }
        ))),
    );
    controller.add_shortcut(shortcut);
    window.add_controller(controller);

    window
}

// TODO: handle unable to spawn tray, do we retry? do we tell user to install appindicator gnome
// extension?
//
// TODO: UI for no service

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

async fn system_bus() -> zbus::Result<zbus::Connection> {
    zbus::connection::Builder::system()?.build().await
}

/// Classify why the obscura service socket is unreachable, by querying systemd (unprivileged reads).
async fn diagnose_linux_service() -> LinuxDegradation {
    let Ok(conn) = system_bus().await else {
        return LinuxDegradation::Stopped;
    };
    let Ok(systemd) = zbus_systemd::systemd1::ManagerProxy::new(&conn).await else {
        return LinuxDegradation::Stopped;
    };
    match systemd.get_unit_file_state("obscura.service".to_owned()).await {
        Err(_) => LinuxDegradation::NotInstalled,
        Ok(state) if state == "disabled" => LinuxDegradation::Disabled,
        Ok(_) => {
            if let Ok(path) = systemd.get_unit("obscura.service".to_owned()).await
                && let Ok(unit) = zbus_systemd::systemd1::UnitProxy::new(&conn, path).await
                && matches!(unit.active_state().await.as_deref(), Ok("failed"))
            {
                return LinuxDegradation::Failed;
            }
            LinuxDegradation::Stopped
        }
    }
}

async fn apply_linux_fix(action: LinuxFixAction) -> Result<(), String> {
    match action {
        LinuxFixAction::Start => systemd_start(false).await,
        LinuxFixAction::EnableAndStart => systemd_start(true).await,
        LinuxFixAction::AddOperator => add_operator().await,
    }
}

async fn systemd_start(enable: bool) -> Result<(), String> {
    let conn = zbus::connection::Builder::system()
        .map_err(|e| e.to_string())?
        .method_timeout(Duration::MAX) // interactive polkit auth can take a while
        .build()
        .await
        .map_err(|e| e.to_string())?;
    let authority = zbus_polkit::policykit1::AuthorityProxy::new(&conn).await.map_err(|e| e.to_string())?;
    let subject = polkit_subject(&conn);
    authorize(&authority, &subject, "org.freedesktop.systemd1.manage-units").await?;
    let systemd = zbus_systemd::systemd1::ManagerProxy::new(&conn).await.map_err(|e| e.to_string())?;
    if enable {
        authorize(&authority, &subject, "org.freedesktop.systemd1.manage-unit-files").await?;
        systemd
            .enable_unit_files(vec!["obscura.service".to_owned()], false, true)
            .await
            .map_err(|e| e.to_string())?;
    }
    let unit = systemd.load_unit("obscura.service".to_owned()).await.map_err(|e| e.to_string())?;
    let unit_proxy = zbus_systemd::systemd1::UnitProxy::new(&conn, unit).await.map_err(|e| e.to_string())?;
    unit_proxy.start("replace".to_owned()).await.map_err(|e| e.to_string())?;
    Ok(())
}

// system-bus-name subject avoids PID-namespace issues in sandboxes (polkit reads the connection's
// credentials), falling back to unix-process when no unique name is available.
fn polkit_subject(conn: &zbus::Connection) -> Subject {
    if let Some(bus_name) = conn.unique_name() {
        use zbus::zvariant::{OwnedValue, Str};
        let mut subject_details = std::collections::HashMap::new();
        subject_details.insert("name".to_string(), OwnedValue::from(Str::from(bus_name.as_str())));
        Subject { subject_kind: "system-bus-name".to_string(), subject_details }
    } else {
        Subject::new_for_owner(std::process::id(), None, None).unwrap()
    }
}

async fn authorize(authority: &zbus_polkit::policykit1::AuthorityProxy<'_>, subject: &Subject, action: &str) -> Result<(), String> {
    let result = authority
        .check_authorization(
            subject,
            action,
            &std::collections::HashMap::new(),
            CheckAuthorizationFlags::AllowUserInteraction.into(),
            "",
        )
        .await
        .map_err(|e| e.to_string())?;
    if result.is_authorized {
        Ok(())
    } else {
        Err(format!("not authorized for {action}"))
    }
}

async fn add_operator() -> Result<(), String> {
    let status = tokio::process::Command::new("pkexec")
        .arg("obscura")
        .arg("add-operator")
        .status()
        .await
        .map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`pkexec obscura add-operator` failed: {status}"))
    }
}

// TODO: handle service stop after gui start

pub(crate) fn main() -> glib::ExitCode {
    if std::env::args().skip(1).any(|arg| arg == "ipc-test") {
        match tokio_rt().block_on(run_command::<()>(ManagerCmd::Ping {})) {
            Ok(Ok(())) => std::process::exit(0),
            _ => std::process::exit(1),
        }
    }

    if !std::env::args().skip(1).any(|arg| arg == "--no-group-refresh") {
        tokio_rt().block_on(obscuravpn_client::linux::try_group_refresh_fix());
    }

    eprintln!("First light");

    let (open_sender, mut open_receiver) = futures::channel::mpsc::unbounded::<()>();

    let osskeeper = Arc::new(OsStatusKeeper::default());
    let tray = MyTray { selected_option: 0, checked: false, os_status: None, open_sender };

    tokio_rt().spawn({
        let osskeeper = osskeeper.clone();
        async move {
            let handle = tray.spawn().await.unwrap();
            tokio::time::sleep(Duration::from_secs(2)).await;

            // We can modify the tray
            handle.update(|tray: &mut MyTray| tray.checked = true).await;

            let mut known_version: Option<Uuid> = None;
            loop {
                let ossfull = osskeeper.wait_next_status(known_version).await;
                known_version = Some(ossfull.version);
                handle.update(|tray: &mut MyTray| tray.os_status = Some(ossfull)).await;
            }
        }
    });

    // So that we can initialize our window without being in connect_activate/startup Fn scope
    // (which is not FnOnce), see: https://gtk-rs.org/gtk4-rs/stable/latest/docs/gtk4/fn.init.html
    let Ok(()) = gtk::init() else {
        eprintln!("Failed to init gtk4");
        return glib::ExitCode::FAILURE;
    };

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
        .flags(gio::ApplicationFlags::default())
        .build();

    let window = build_primary_window(osskeeper);

    glib::spawn_future_local(glib::clone!(
        #[strong]
        window,
        async move {
            while let Some(()) = open_receiver.next().await {
                window.present();
            }
        }
    ));

    app.connect_startup(glib::clone!(
        #[strong]
        window,
        move |app| {
            app.add_window(&window);
        }
    ));

    app.connect_activate(glib::clone!(
        #[strong]
        window,
        move |_app| {
            window.present();
        }
    ));

    tokio_to_glib_local_fut_pipe(
        async move {
            tokio::signal::ctrl_c().await.expect("failed to listen for ctrl-c");
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

    app.run_with_args::<&str>(&[])
}

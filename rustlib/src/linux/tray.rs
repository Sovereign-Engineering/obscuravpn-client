use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use anyhow::Context as _;
use futures::channel::mpsc::{Receiver, Sender, channel};
use futures::{FutureExt as _, StreamExt as _};
use ksni::menu::{CheckmarkItem, MenuItem, StandardItem, SubMenu};
use ksni::{Icon, OfflineReason, Status, ToolTip, Tray, TrayMethods};
use obscuravpn_api::cmd::ExitList;
use obscuravpn_api::types::{CityCode, CountryCode};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg;
use serde_json::Value;
use tokio::runtime::Handle;
use tokio::time::{sleep, timeout};
use zbus::names::BusName;
use zbus::proxy::CacheProperties;

use super::exit_list_watch::GuiExitListWatch;
use super::ipc::run_command;
use super::status::{LinuxServiceDegradation, NEVPNStatus, OsStatus, ServiceStatus};
use super::status_watch::GuiStatusWatch;
use crate::exit_selection::ExitSelector;
use crate::manager::{TunnelArgs, VpnStatus};
use crate::manager_cmd::ManagerCmd;

pub enum ShowTarget {
    MainWindow,
    LocationView,
}

pub enum TrayRequest {
    Show(ShowTarget),
    Quit,
}

static ICONS: LazyLock<TrayIcons> = LazyLock::new(|| TrayIcons::render().expect("embedded tray icon svgs must render"));

struct TrayIcons {
    disconnected: Vec<Icon>,
    connecting: [Vec<Icon>; 3],
    connected: Vec<Icon>,
    needs_attention: Vec<Icon>,
}

impl TrayIcons {
    fn render() -> anyhow::Result<Self> {
        Ok(Self {
            disconnected: Self::render_icon_sizes(include_str!("../../tray-icons/Disconnected-light.svg"))?,
            connecting: [
                Self::render_icon_sizes(include_str!("../../tray-icons/Connecting-1-light.svg"))?,
                Self::render_icon_sizes(include_str!("../../tray-icons/Connecting-2-light.svg"))?,
                Self::render_icon_sizes(include_str!("../../tray-icons/Connecting-3-light.svg"))?,
            ],
            connected: Self::render_icon_sizes(include_str!("../../tray-icons/Connected-light.svg"))?,
            needs_attention: Self::render_icon_sizes(include_str!("../../tray-icons/NeedsAttention-light.svg"))?,
        })
    }

    fn render_icon_sizes(svg: &str) -> anyhow::Result<Vec<Icon>> {
        let tree = usvg::Tree::from_str(svg, &usvg::Options::default()).context("failed to parse tray icon svg")?;
        let mut icons = Vec::new();
        for size in [16u16, 22, 24, 32, 48] {
            let mut pixmap = Pixmap::new(u32::from(size), u32::from(size)).context("failed to allocate tray icon pixmap")?;
            let transform = Transform::from_scale(f32::from(size) / tree.size().width(), f32::from(size) / tree.size().height());
            resvg::render(&tree, transform, &mut pixmap.as_mut());
            let mut data = Vec::with_capacity(pixmap.data().len());
            for pixel in pixmap.pixels() {
                let color = pixel.demultiply();
                data.extend_from_slice(&[color.alpha(), color.red(), color.green(), color.blue()]);
            }
            icons.push(Icon { width: i32::from(size), height: i32::from(size), data });
        }
        Ok(icons)
    }
}

fn status_line(os_status: &OsStatus) -> String {
    match &os_status.service_status {
        ServiceStatus::Initializing => "Starting...".to_owned(),
        ServiceStatus::Degraded { last_status: _, linux_degradation } => match linux_degradation {
            LinuxServiceDegradation::UnitInactive => "Service not running".to_owned(),
            LinuxServiceDegradation::UnitActivating => "Service starting...".to_owned(),
            LinuxServiceDegradation::UnitNotInstalled => "Service not installed".to_owned(),
            LinuxServiceDegradation::SocketPermissionDenied { user: _ } => "Service access denied".to_owned(),
            LinuxServiceDegradation::VersionMismatch { service_version: _, app_version: _, installed_app_version_differs: _ } => {
                "Service version differs from app".to_owned()
            }
            LinuxServiceDegradation::Unknown => "Service unreachable".to_owned(),
        },
        ServiceStatus::Healthy(status) => match &status.vpn_status {
            VpnStatus::Disconnected {} => "Disconnected".to_owned(),
            VpnStatus::Connecting { tunnel_args: _, connect_error: _, reconnecting: _ } => "Connecting...".to_owned(),
            VpnStatus::Connected { tunnel_args: _, exit, relay: _, client_public_key: _, exit_public_key: _, transport: _ } => {
                format!("Connected to {}, {}", exit.city_name, exit.city_code.country_code.0.to_uppercase())
            }
        },
    }
}

fn selector_is_city(selector: &ExitSelector, country_code: &str, city_code: &str) -> bool {
    match selector {
        ExitSelector::City { city_code: selected } => selected.country_code.0 == country_code && selected.city_code == city_code,
        ExitSelector::Any {} | ExitSelector::Exit { id: _ } | ExitSelector::Country { country_code: _ } => false,
    }
}

struct TrayState {
    os_status: OsStatus,
    connecting_frame: usize,
    requests: Sender<TrayRequest>,
    exit_list: Option<Arc<ExitList>>,
    rt: Handle,
}

impl TrayState {
    fn request(&mut self, request: TrayRequest) {
        if let Err(error) = self.requests.try_send(request) {
            tracing::error!(message_id = "Ke4jXs6r", %error, "failed to send window request: {error}");
        }
    }

    fn spawn_manager_cmd(&self, cmd: ManagerCmd) {
        self.rt.spawn(async move {
            let logged_cmd = cmd.clone();
            match run_command::<Value>(cmd).await {
                Ok(Ok(_)) => {}
                Ok(Err(error)) => tracing::error!(message_id = "Mh6bTn2w", cmd = ?logged_cmd, ?error, "tray manager command failed"),
                Err(error) => tracing::error!(message_id = "Vd4kRp7c", cmd = ?logged_cmd, ?error, "tray manager command failed"),
            }
        });
    }

    fn quit_and_disconnect(&mut self) {
        let mut requests = self.requests.clone();
        self.rt.spawn(async move {
            let cmd = ManagerCmd::SetTunnelArgs { args: None, active: Some(false) };
            match timeout(Duration::from_secs(5), run_command::<Value>(cmd)).await {
                Ok(Ok(Ok(_))) => {}
                Ok(Ok(Err(error))) => tracing::error!(message_id = "Vf6cQj2n", ?error, "disconnect on quit failed"),
                Ok(Err(error)) => tracing::error!(message_id = "Pk8sYw3m", ?error, "disconnect on quit failed"),
                Err(_) => tracing::error!(message_id = "Rt5dZb7q", "disconnect on quit timed out"),
            }
            if let Err(error) = requests.try_send(TrayRequest::Quit) {
                tracing::error!(message_id = "Uw2gNv9x", %error, "failed to send quit request: {error}");
            }
        });
    }

    fn location_submenu(&self) -> MenuItem<Self> {
        let status = match &self.os_status.service_status {
            ServiceStatus::Initializing => None,
            ServiceStatus::Healthy(status) => Some(status),
            ServiceStatus::Degraded { last_status, linux_degradation: _ } => last_status.as_ref(),
        };
        let last_exit = status.map(|status| &status.last_exit);
        let city_names: Option<HashMap<(&str, &str), &str>> = self.exit_list.as_deref().map(|exit_list| {
            exit_list
                .exits
                .iter()
                .map(|exit| {
                    (
                        (exit.city_code.country_code.0.as_str(), exit.city_code.city_code.as_str()),
                        exit.city_name.as_str(),
                    )
                })
                .collect()
        });

        let mut submenu: Vec<MenuItem<Self>> = Vec::new();

        let quick_connect_checked = match last_exit {
            Some(ExitSelector::Any {}) => true,
            Some(ExitSelector::Exit { id: _ } | ExitSelector::Country { country_code: _ } | ExitSelector::City { city_code: _ }) | None => false,
        };
        submenu.push(
            CheckmarkItem {
                label: "Quick Connect".to_owned(),
                checked: quick_connect_checked,
                activate: Box::new(|this: &mut Self| {
                    this.spawn_manager_cmd(ManagerCmd::SetTunnelArgs { args: Some(TunnelArgs { exit: ExitSelector::Any {} }), active: Some(true) })
                }),
                ..Default::default()
            }
            .into(),
        );

        submenu.push(StandardItem { label: "Pinned Locations".to_owned(), enabled: false, ..Default::default() }.into());

        let mut shown_any_pinned = false;
        let mut last_exit_is_pinned = false;
        for pinned in status.map(|status| status.pinned_locations.as_slice()).unwrap_or_default() {
            let city_name = match &city_names {
                Some(city_names) => match city_names.get(&(pinned.country_code.as_str(), pinned.city_code.as_str())) {
                    Some(city_name) => (*city_name).to_owned(),
                    None => continue,
                },
                None => pinned.city_code.clone(),
            };
            let checked = last_exit.is_some_and(|selector| selector_is_city(selector, &pinned.country_code, &pinned.city_code));
            last_exit_is_pinned |= checked;
            let selector = ExitSelector::City {
                city_code: CityCode { country_code: CountryCode(pinned.country_code.clone()), city_code: pinned.city_code.clone() },
            };
            submenu.push(
                CheckmarkItem {
                    label: format!("{city_name}, {}", pinned.country_code.to_uppercase()),
                    checked,
                    activate: Box::new(move |this: &mut Self| {
                        this.spawn_manager_cmd(ManagerCmd::SetTunnelArgs { args: Some(TunnelArgs { exit: selector.clone() }), active: Some(true) })
                    }),
                    ..Default::default()
                }
                .into(),
            );
            shown_any_pinned = true;
        }
        if !shown_any_pinned {
            submenu.push(StandardItem { label: "Pinned locations will appear here".to_owned(), enabled: false, ..Default::default() }.into());
        }

        if let Some(ExitSelector::City { city_code }) = last_exit
            && !last_exit_is_pinned
        {
            let city_name = city_names
                .as_ref()
                .and_then(|city_names| {
                    city_names
                        .get(&(city_code.country_code.0.as_str(), city_code.city_code.as_str()))
                        .copied()
                })
                .unwrap_or(city_code.city_code.as_str());
            let selector = ExitSelector::City { city_code: city_code.clone() };
            submenu.push(StandardItem { label: "Current Selection".to_owned(), enabled: false, ..Default::default() }.into());
            submenu.push(
                CheckmarkItem {
                    label: format!("{city_name}, {}", city_code.country_code.0.to_uppercase()),
                    checked: true,
                    activate: Box::new(move |this: &mut Self| {
                        this.spawn_manager_cmd(ManagerCmd::SetTunnelArgs { args: Some(TunnelArgs { exit: selector.clone() }), active: Some(true) })
                    }),
                    ..Default::default()
                }
                .into(),
            );
        }

        submenu.push(MenuItem::Separator);
        submenu.push(
            StandardItem {
                label: "More Locations...".to_owned(),
                activate: Box::new(|this: &mut Self| this.request(TrayRequest::Show(ShowTarget::LocationView))),
                ..Default::default()
            }
            .into(),
        );

        let enabled = self.exit_list.is_some()
            && match &self.os_status.service_status {
                ServiceStatus::Healthy(_) => true,
                ServiceStatus::Initializing | ServiceStatus::Degraded { last_status: _, linux_degradation: _ } => false,
            };
        SubMenu { label: "Connect via...".to_owned(), enabled, submenu, ..Default::default() }.into()
    }
}

impl Tray for TrayState {
    const MENU_ON_ACTIVATE: bool = true;

    fn id(&self) -> String {
        "net.obscura.vpn.gui".to_owned()
    }

    fn title(&self) -> String {
        "Obscura VPN".to_owned()
    }

    fn status(&self) -> Status {
        match &self.os_status.service_status {
            ServiceStatus::Degraded { last_status: _, linux_degradation: _ } => Status::NeedsAttention,
            ServiceStatus::Initializing | ServiceStatus::Healthy(_) => Status::Active,
        }
    }

    fn attention_icon_pixmap(&self) -> Vec<Icon> {
        ICONS.needs_attention.clone()
    }

    fn icon_pixmap(&self) -> Vec<Icon> {
        match &self.os_status.service_status {
            ServiceStatus::Initializing | ServiceStatus::Degraded { last_status: _, linux_degradation: _ } => ICONS.needs_attention.clone(),
            ServiceStatus::Healthy(status) => match &status.vpn_status {
                VpnStatus::Disconnected {} => ICONS.disconnected.clone(),
                VpnStatus::Connecting { tunnel_args: _, connect_error: _, reconnecting: _ } => ICONS.connecting[self.connecting_frame].clone(),
                VpnStatus::Connected { tunnel_args: _, exit: _, relay: _, client_public_key: _, exit_public_key: _, transport: _ } => {
                    ICONS.connected.clone()
                }
            },
        }
    }

    fn tool_tip(&self) -> ToolTip {
        ToolTip {
            icon_name: String::new(),
            icon_pixmap: Vec::new(),
            title: "Obscura VPN".to_owned(),
            description: status_line(&self.os_status),
        }
    }

    fn menu(&self) -> Vec<MenuItem<Self>> {
        let mut items: Vec<MenuItem<Self>> = vec![
            StandardItem { label: status_line(&self.os_status), enabled: false, ..Default::default() }.into(),
            MenuItem::Separator,
        ];
        items.push(
            match &self.os_status.service_status {
                ServiceStatus::Initializing | ServiceStatus::Degraded { last_status: _, linux_degradation: _ } => {
                    StandardItem { label: "Connect".to_owned(), enabled: false, ..Default::default() }
                }
                ServiceStatus::Healthy(status) => match &status.vpn_status {
                    VpnStatus::Disconnected {} => StandardItem {
                        label: "Connect".to_owned(),
                        activate: Box::new(|this: &mut Self| this.spawn_manager_cmd(ManagerCmd::SetTunnelArgs { args: None, active: Some(true) })),
                        ..Default::default()
                    },
                    VpnStatus::Connecting { tunnel_args: _, connect_error: _, reconnecting: _ }
                    | VpnStatus::Connected { tunnel_args: _, exit: _, relay: _, client_public_key: _, exit_public_key: _, transport: _ } => {
                        StandardItem {
                            label: "Disconnect".to_owned(),
                            activate: Box::new(|this: &mut Self| {
                                this.spawn_manager_cmd(ManagerCmd::SetTunnelArgs { args: None, active: Some(false) })
                            }),
                            ..Default::default()
                        }
                    }
                },
            }
            .into(),
        );
        items.push(self.location_submenu());
        items.push(MenuItem::Separator);
        items.push(
            StandardItem {
                label: "Open Obscura Manager...".to_owned(),
                activate: Box::new(|this: &mut Self| this.request(TrayRequest::Show(ShowTarget::MainWindow))),
                ..Default::default()
            }
            .into(),
        );
        items.push(MenuItem::Separator);
        items.push(StandardItem { label: self.os_status.src_version.to_owned(), enabled: false, ..Default::default() }.into());
        items.push(
            StandardItem {
                label: "Quit and Disconnect".to_owned(),
                activate: Box::new(|this: &mut Self| this.quit_and_disconnect()),
                ..Default::default()
            }
            .into(),
        );
        items
    }

    fn watcher_online(&self) {
        tracing::info!(message_id = "Nw5hG7pt", "StatusNotifierWatcher online, tray available");
    }

    fn watcher_offline(&self, reason: OfflineReason) -> bool {
        tracing::warn!(
            message_id = "Qy3zM6es",
            ?reason,
            "StatusNotifierWatcher offline, tray hidden until it returns"
        );
        true
    }
}

#[zbus::proxy(
    interface = "org.kde.StatusNotifierWatcher",
    default_service = "org.kde.StatusNotifierWatcher",
    default_path = "/StatusNotifierWatcher"
)]
trait StatusNotifierWatcher {
    #[zbus(property)]
    fn registered_status_notifier_items(&self) -> zbus::Result<Vec<String>>;

    #[zbus(signal)]
    fn status_notifier_item_unregistered(&self, service: &str) -> zbus::Result<()>;
}

async fn any_abandoned_tray(watcher: &StatusNotifierWatcherProxy<'_>, dbus: &zbus::fdo::DBusProxy<'_>) -> anyhow::Result<bool> {
    for item in watcher.registered_status_notifier_items().await? {
        let Ok(bus_name) = BusName::try_from(item.split(['/', '@']).next().unwrap_or_default()) else {
            continue;
        };
        if !dbus.name_has_owner(bus_name).await? {
            return Ok(true);
        }
    }
    Ok(false)
}

async fn wait_until_abandoned_trays_dropped(max_wait: Duration) {
    let wait = async {
        let connection = zbus::Connection::session().await?;
        let watcher = StatusNotifierWatcherProxy::builder(&connection)
            .cache_properties(CacheProperties::No)
            .build()
            .await?;
        let dbus = zbus::fdo::DBusProxy::new(&connection).await?;
        let mut unregistered = watcher.receive_status_notifier_item_unregistered().await?;
        while any_abandoned_tray(&watcher, &dbus).await? {
            tracing::info!(message_id = "Bq7wVn3k", "waiting for abandoned tray registration to be dropped");
            unregistered.next().await;
        }
        anyhow::Ok(())
    };
    match timeout(max_wait, wait).await {
        Ok(Ok(())) => {}
        Ok(Err(error)) => tracing::debug!(message_id = "Zr4pLd8t", ?error, "could not check for abandoned tray registrations"),
        Err(_) => tracing::warn!(message_id = "Mx2cHf6s", "timed out waiting for abandoned tray registration to be dropped"),
    }
}

pub async fn spawn_tray(status: Arc<GuiStatusWatch>, exit_list: Arc<GuiExitListWatch>) -> Receiver<TrayRequest> {
    LazyLock::force(&ICONS);
    let (requests, receiver) = channel(1);
    tokio::spawn(run_tray(status, exit_list, requests));
    receiver
}

async fn run_tray(status: Arc<GuiStatusWatch>, exit_list: Arc<GuiExitListWatch>, requests: Sender<TrayRequest>) {
    // Avoid briefly showing two icons after restart via exec, because GNOME appindicator keeps abandoned trays registered briefly.
    wait_until_abandoned_trays_dropped(Duration::from_secs(1)).await;
    let handle = loop {
        let tray = TrayState {
            os_status: OsStatus::default(),
            connecting_frame: 0,
            requests: requests.clone(),
            exit_list: None,
            rt: Handle::current(),
        };
        let tray = tray
            .assume_sni_available(true)
            // The default PID-based name repeats after restart via exec and may collide, because GNOME appindicator keeps abandoned trays registered briefly.
            .disable_dbus_name(true);
        match tray.spawn().await {
            Ok(handle) => break handle,
            Err(error) => {
                tracing::error!(message_id = "Ah9tBc5y", %error, "failed to start system tray, retrying in 30s");
                sleep(Duration::from_secs(30)).await;
            }
        }
    };
    let exit_list_handle = handle.clone();
    tokio::spawn(async move {
        let mut known: Option<Arc<ExitList>> = None;
        loop {
            known = Some(exit_list.changed(known.as_ref()).await);
            let Some(()) = exit_list_handle.update(|tray| tray.exit_list = known.clone()).await else {
                tracing::error!(message_id = "Wt6qJm3z", "tray service stopped");
                return;
            };
        }
    });
    tokio::spawn(async move {
        let mut known_version = None;
        let mut connecting = false;
        let mut frame: usize = 0;
        loop {
            let new_status = if connecting {
                frame = (frame + 1) % 3;
                status.changed(known_version).now_or_never()
            } else {
                frame = 0;
                Some(status.changed(known_version).await)
            };
            if let Some(new_status) = &new_status {
                known_version = Some(new_status.version);
                connecting = new_status.os_vpn_status == NEVPNStatus::Connecting;
            }
            let Some(()) = handle
                .update(|tray| {
                    if let Some(new_status) = new_status {
                        tray.os_status = new_status;
                    }
                    tray.connecting_frame = frame;
                })
                .await
            else {
                tracing::error!(message_id = "Sy7fKd4v", "tray service stopped");
                return;
            };
            // Space out NewIcon signals to prevent the GNOME appindicator extension from dropping updates.
            sleep(Duration::from_millis(500)).await;
        }
    });
}

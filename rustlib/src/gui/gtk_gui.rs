use crate::auto_connect::auto_connect_if_enabled;
use crate::webview::build_webview;
use crate::webview_cmd::WebviewCmdContext;
use crate::{GtkAppFinished, MainThreadToken};
use futures::StreamExt;
use futures::channel::mpsc::Receiver;
use libadwaita::StyleManager;
use obscuravpn_client::linux::debug_bundle::GuiDebugBundler;
use obscuravpn_client::linux::status::NavigationView;
use obscuravpn_client::linux::status_watch::GuiStatusWatch;
use obscuravpn_client::linux::tray::{ShowTarget, TrayRequest};
use obscuravpn_client::ui_config::{ColorScheme, UiConfigHandle};
use std::cell::Cell;
use std::future::Future;
use std::process::ExitCode;
use std::rc::Rc;
use std::sync::Arc;
use strum::IntoEnumIterator;
use tokio::sync::watch;
use webkit6::gtk::{self, Align, Label, ListBox, Orientation, SelectionMode};
use webkit6::{WebView, gdk, gio, glib, prelude::*};

/// Proof that GTK 4 and libadwaita are initialized. Neither Send nor Sync, so it cannot leave the main thread where initialization happened.
#[derive(Clone, Copy)]
pub(crate) struct GtkInitToken {
    main_thread: MainThreadToken,
}

static_assertions::assert_not_impl_any!(GtkInitToken: Send, Sync);

impl GtkInitToken {
    fn init(main_thread: MainThreadToken) -> Option<Self> {
        let Ok(()) = gtk::init() else {
            tracing::error!(message_id = "Sd2vLq7M", "failed to init gtk4");
            return None;
        };
        let Ok(()) = libadwaita::init() else {
            tracing::error!(message_id = "Yb8nFt3K", "failed to init libadwaita");
            return None;
        };
        Some(Self { main_thread })
    }

    pub(crate) fn main_thread(self) -> MainThreadToken {
        self.main_thread
    }
}

/// Must run on the main thread. Blocks until the app quits.
pub(crate) fn run_gtk_app(
    main_thread: MainThreadToken,
    gui_status: Arc<GuiStatusWatch>,
    mut tray_receiver: Receiver<TrayRequest>,
    debug_bundler: Arc<GuiDebugBundler>,
    urls: Vec<String>,
    ui_config: Arc<UiConfigHandle>,
) -> GtkAppFinished {
    let Some(gtk_init) = GtkInitToken::init(main_thread) else {
        return GtkAppFinished::Exit(ExitCode::FAILURE);
    };

    register_gresource(include_bytes!(concat!(env!("OBSCURA_GRESOURCES_DIR"), "/icons.gresource")));
    register_gresource(include_bytes!(concat!(env!("OBSCURA_GRESOURCES_DIR"), "/webui.gresource")));

    let app = gtk::Application::builder()
        .application_id("net.obscura.vpn.gui")
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    let (color_scheme_tx, mut color_scheme_rx) = watch::channel(ui_config.get().color_scheme);
    let (restart_tx, restart_rx) = watch::channel(false);
    let (page_ready_tx, page_ready) = watch::channel(false);
    let command_context = WebviewCmdContext {
        gui_status: gui_status.clone(),
        debug_bundler,
        ui_config,
        color_scheme: color_scheme_tx,
        restart: restart_tx,
        page_ready: page_ready_tx,
    };
    let (window, sidebar, webview) = build_primary_window(gtk_init, command_context);

    spawn_on_main_thread(main_thread, async move {
        loop {
            StyleManager::default().set_color_scheme(match *color_scheme_rx.borrow_and_update() {
                ColorScheme::Auto => libadwaita::ColorScheme::Default,
                ColorScheme::Light => libadwaita::ColorScheme::ForceLight,
                ColorScheme::Dark => libadwaita::ColorScheme::ForceDark,
            });
            let Ok(()) = color_scheme_rx.changed().await else { return };
        }
    });

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
                match i32::try_from(status.navigation_view.index())
                    .ok()
                    .and_then(|index| sidebar.row_at_index(index))
                {
                    Some(row) => sidebar.select_row(Some(&row)),
                    None => tracing::warn!(message_id = "Cp7gV3ol", view = %status.navigation_view, "no sidebar row for view"),
                }
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
            tokio::spawn(auto_connect_if_enabled(gui_status.clone()));
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
        webview,
        #[strong]
        page_ready,
        move |_app, files, _hint| {
            for file in files {
                open_app_url(main_thread, &file.uri(), &window, &gui_status, &webview, &page_ready);
            }
        }
    ));

    let mut restart_wait = restart_rx.clone();
    spawn_on_main_thread(
        main_thread,
        glib::clone!(
            #[strong]
            app,
            async move {
                tokio::select! {
                    () = async {
                        if let Err(error) = tokio::signal::ctrl_c().await {
                            tracing::error!(message_id = "Vg5tMc2Q", %error, "failed to listen for ctrl-c, it will not quit the app gracefully");
                            std::future::pending::<()>().await;
                        }
                    } => {}
                    _ = restart_wait.wait_for(|restart| *restart) => {}
                }
                tracing::info!(message_id = "Pv4cRb8T", "quitting gtk app");
                app.quit();
            }
        ),
    );

    let mut gtk_args = vec!["obscura-gui".to_string()];
    gtk_args.extend(urls);
    let gtk_exit_code = app.run_with_args(&gtk_args);
    if *restart_rx.borrow() {
        GtkAppFinished::Restart
    } else {
        GtkAppFinished::Exit(u8::try_from(gtk_exit_code.value()).map(ExitCode::from).unwrap_or(ExitCode::FAILURE))
    }
}

pub(crate) fn spawn_on_main_thread(_main_thread: MainThreadToken, fut: impl Future<Output = ()> + 'static) {
    glib::spawn_future_local(fut);
}

fn register_gresource(bytes: &'static [u8]) {
    let gbytes = glib::Bytes::from_static(bytes);
    let res = gio::Resource::from_data(&gbytes).expect("Could not load gresource file");
    gio::resources_register(&res);
}

fn open_app_url(
    main_thread: MainThreadToken,
    uri: &str,
    window: &gtk::ApplicationWindow,
    gui_status: &GuiStatusWatch,
    webview: &WebView,
    page_ready: &watch::Receiver<bool>,
) {
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
            let webview = webview.clone();
            let mut page_ready = page_ready.clone();
            spawn_on_main_thread(main_thread, async move {
                let Ok(_) = page_ready.wait_for(|ready| *ready).await else { return };
                let script = r#"window.dispatchEvent(new CustomEvent("paymentSucceeded"));"#;
                if let Err(error) = webview.evaluate_javascript_future(script, None, None).await {
                    tracing::error!(message_id = "Hs5cYw7N", %error, "failed to dispatch paymentSucceeded event to webview");
                }
            });
        }
        path => tracing::error!(message_id = "Gt4kLp2M", %path, "unknown app URL path"),
    }
}

fn build_primary_window(gtk_init: GtkInitToken, command_context: WebviewCmdContext) -> (gtk::ApplicationWindow, ListBox, WebView) {
    let window = gtk::ApplicationWindow::builder()
        .title("Obscura VPN")
        .hide_on_close(true)
        .default_width(800)
        .default_height(600)
        .build();

    let display = gdk::Display::default().expect("Could not get default display");
    let icon_theme = gtk::IconTheme::for_display(&display);
    icon_theme.add_resource_path("/com/obscura/vpn/icons/icons");

    let dev_visible = Rc::new(Cell::new(false));

    let gui_status = command_context.gui_status.clone();
    let webview = build_webview(gtk_init, command_context);
    webview.set_hexpand(true);
    let sidebar = build_sidebar(gui_status, dev_visible.clone());

    let split_view = gtk::Box::new(Orientation::Horizontal, 0);
    split_view.append(&sidebar);
    split_view.append(&webview);
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

fn build_sidebar(gui_status: Arc<GuiStatusWatch>, dev_visible: Rc<Cell<bool>>) -> ListBox {
    let list = ListBox::builder()
        .selection_mode(SelectionMode::Browse)
        .css_classes(["navigation-sidebar", "sidebar"])
        .build();

    for view in NavigationView::iter() {
        list.append(&view_row_widget(view));
    }

    list.set_filter_func(move |row| match usize::try_from(row.index()).ok().and_then(NavigationView::from_index) {
        Some(NavigationView::Developer) => dev_visible.get(),
        Some(
            NavigationView::Connection
            | NavigationView::Location
            | NavigationView::Account
            | NavigationView::Settings
            | NavigationView::Help
            | NavigationView::About,
        )
        | None => true,
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

        let Some(view) = usize::try_from(lbr.index()).ok().and_then(NavigationView::from_index) else {
            return;
        };
        gui_status.set_navigation_view(view);
    });

    list
}

fn view_row_widget(view: NavigationView) -> gtk::Box {
    let hbox = gtk::Box::new(Orientation::Horizontal, 8);
    let icon = gtk::Image::from_icon_name(match view {
        NavigationView::Connection => "obscura-connection-symbolic",
        NavigationView::Location => "obscura-location-symbolic",
        NavigationView::Account => "obscura-account-symbolic",
        NavigationView::Settings => "obscura-settings-symbolic",
        NavigationView::Help => "obscura-help-symbolic",
        NavigationView::About => "obscura-about-symbolic",
        NavigationView::Developer => "obscura-developer-symbolic",
    });
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

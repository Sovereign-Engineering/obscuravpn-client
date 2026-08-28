use crate::gtk_gui::{GtkInitToken, spawn_on_main_thread};
use crate::webview_cmd::WebviewCmdContext;
use webkit6::{
    HardwareAccelerationPolicy, Settings, URISchemeRequest, UserContentInjectedFrames, UserScript, WebContext, WebView, gio, glib, javascriptcore,
    prelude::*,
};

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
function fmt(arg) {
    try {
        return arg instanceof Error ? String(arg) : JSON.stringify(arg, undefined, "\t");
    } catch (e) {
        return String(arg);
    }
}
function log(type, msg, ...args) {
    let formatted = [type, msg, ...args.map(fmt)].join(" ");
    window.webkit.messageHandlers.logBridge.postMessage(formatted);
}
console.debug = log.bind(null, "debug:");
console.log = log.bind(null, "log:");
console.warn = log.bind(null, "warn:");
console.error = log.bind(null, "error:");
"#;

pub(crate) fn build_webview(gtk_init: GtkInitToken, command_context: WebviewCmdContext) -> WebView {
    let user_content_manager = webkit6::UserContentManager::new();

    for capture_script in [JS_ERROR_CAPTURE, JS_LOG_CAPTURE] {
        let script = UserScript::new(
            capture_script,
            UserContentInjectedFrames::AllFrames,
            webkit6::UserScriptInjectionTime::Start,
            &[],
            &[],
        );
        user_content_manager.add_script(&script);
    }

    let page_ready = command_context.page_ready.clone();
    user_content_manager.connect_script_message_with_reply_received(Some("commandBridge"), move |_ucm, value, reply| {
        spawn_on_main_thread(
            gtk_init.main_thread(),
            command_context.clone().handle_command_json(value.clone(), reply.clone()),
        );
        true
    });
    user_content_manager.register_script_message_handler_with_reply("commandBridge", None);

    user_content_manager.connect_script_message_received(Some("errorBridge"), forward_js_error);
    user_content_manager.register_script_message_handler("errorBridge", None);

    user_content_manager.connect_script_message_received(Some("logBridge"), forward_console_message);
    user_content_manager.register_script_message_handler("logBridge", None);

    let settings = Settings::builder()
        .enable_developer_extras(true)
        .hardware_acceleration_policy(HardwareAccelerationPolicy::Never)
        .build();

    let context = WebContext::new();
    context.register_uri_scheme("web-ui", serve_web_ui_resource);

    let webview = WebView::builder()
        .settings(&settings)
        .user_content_manager(&user_content_manager)
        .web_context(&context)
        .build();

    webview.connect_decide_policy(decide_policy);

    webview.connect_load_changed(move |_webview, event| {
        if event == webkit6::LoadEvent::Started {
            page_ready.send_replace(false);
        }
    });

    webview.connect_web_process_terminated(|webview, reason| {
        tracing::error!(message_id = "Fq2xNr7V", ?reason, "webview process terminated, reloading in 1s");
        let webview = webview.clone();
        glib::timeout_add_local_once(std::time::Duration::from_secs(1), move || webview.reload());
    });

    webview.load_uri("web-ui:///index.html");

    webview
}

fn serve_web_ui_resource(request: &URISchemeRequest) {
    let Some(uri) = request.uri() else {
        tracing::error!(message_id = "Wq4dNs7K", "web-ui request has no URI");
        finish_with_not_found(request);
        return;
    };
    let Some(path) = uri.strip_prefix("web-ui://") else {
        tracing::error!(message_id = "Mh8tVc3P", %uri, "web-ui request URI has unexpected prefix");
        finish_with_not_found(request);
        return;
    };
    let gfile = gio::File::for_uri(&format!("resource:///com/obscura/vpn/web-ui{path}"));
    let mimetype = match gfile.query_info(
        gio::FILE_ATTRIBUTE_STANDARD_CONTENT_TYPE,
        gio::FileQueryInfoFlags::NONE,
        gio::Cancellable::NONE,
    ) {
        Ok(info) => info.content_type(),
        Err(error) => {
            tracing::error!(message_id = "Zf6rQm2X", %uri, %error, "failed to query web-ui resource info");
            request.finish_error(&mut error.clone());
            return;
        }
    };
    match gfile.read(gio::Cancellable::NONE) {
        Ok(stream) => request.finish(&stream, -1, mimetype.as_ref().map(|mimetype| mimetype.as_str())),
        Err(error) => {
            tracing::error!(message_id = "Ck5wHb9S", %uri, %error, "failed to open web-ui resource");
            request.finish_error(&mut error.clone());
        }
    }
}

fn finish_with_not_found(request: &URISchemeRequest) {
    request.finish_error(&mut glib::Error::new(gio::IOErrorEnum::NotFound, "resource not found"));
}

fn decide_policy(_webview: &WebView, decision: &webkit6::PolicyDecision, _decision_type: webkit6::PolicyDecisionType) -> bool {
    let Some(nav_decision) = decision.downcast_ref::<webkit6::NavigationPolicyDecision>() else {
        return false;
    };
    let Some(mut nav_action) = nav_decision.navigation_action() else {
        return false;
    };
    let webkit6::NavigationType::LinkClicked = nav_action.navigation_type() else {
        return false;
    };
    let Some(request) = nav_action.request() else {
        return false;
    };
    let Some(uri) = request.uri() else {
        return false;
    };

    tracing::info!(message_id = "Ub3fSk7D", %uri, "opening clicked link externally");
    if let Err(error) = open::that_detached(uri.as_str()) {
        tracing::error!(message_id = "Ap6jXe2N", %uri, %error, "failed to open link externally");
    }

    decision.ignore();
    true
}

fn forward_js_error(_ucm: &webkit6::UserContentManager, value: &javascriptcore::Value) {
    tracing::error!(message_id = "Yc6vBn2W", error = %value.to_str(), "webview error");
}

fn forward_console_message(_ucm: &webkit6::UserContentManager, value: &javascriptcore::Value) {
    let message = value.to_str();
    let message = message.as_str();
    if let Some(message) = message.strip_prefix("error:") {
        tracing::error!(message_id = "Dt3kWq8M", "webview console: {}", message.trim_start());
    } else if let Some(message) = message.strip_prefix("warn:") {
        tracing::warn!(message_id = "Gs5xPk2V", "webview console: {}", message.trim_start());
    } else if let Some(message) = message.strip_prefix("debug:") {
        tracing::debug!(message_id = "Lb7cJf4T", "webview console: {}", message.trim_start());
    } else {
        let message = message.strip_prefix("log:").unwrap_or(message);
        tracing::info!(message_id = "Rn2mZd6H", "webview console: {}", message.trim_start());
    }
}

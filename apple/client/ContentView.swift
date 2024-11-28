import OSLog
import SwiftUI
import UniformTypeIdentifiers
import WebKit

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "ContentView")

struct NavView: Hashable {
    let name: String
    let systemImageName: String
    func hash(into hasher: inout Hasher) {
        hasher.combine(self.name)
    }
}

let STABLE_VIEWS = [
    NavView(name: "connection", systemImageName: "network.badge.shield.half.filled"),
    NavView(name: "location", systemImageName: "mappin.and.ellipse"),
    NavView(name: "account", systemImageName: "person.circle"),
    NavView(name: "help", systemImageName: "questionmark.circle"),
    NavView(name: "settings", systemImageName: "gear"),
]

let EXPERIMETNAL_VIEWS: [NavView] = [
]

let DEBUG_VIEWS = [
    NavView(name: "developer", systemImageName: "book.and.wrench"),
]

let VIEW_MODES = [
    STABLE_VIEWS,
    STABLE_VIEWS + DEBUG_VIEWS,
    STABLE_VIEWS + EXPERIMETNAL_VIEWS + DEBUG_VIEWS,
]

#if DEBUG
    let DEFAULT_VIEW_MODE = VIEW_MODES.count - 1
#else
    let DEFAULT_VIEW_MODE = 0
#endif

class ViewModeManager: ObservableObject {
    @Published private var viewIndex = DEFAULT_VIEW_MODE
    private var eventMonitor: Any?

    init() {
        self.eventMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
            if event.charactersIgnoringModifiers == "D" && event.modifierFlags.contains(.command) {
                // Cmd+Shift+d
                self.viewIndex = (self.viewIndex + 1) % VIEW_MODES.count
                return nil
            }
            return event
        }
    }

    deinit {
        NSEvent.removeMonitor(self.eventMonitor)
    }

    func getViews() -> [NavView] {
        return VIEW_MODES[self.viewIndex]
    }
}

func getBadgeText(_ daysToExpiry: Int64) -> String {
    if daysToExpiry > 3 {
        return "expires soon"
    }
    if daysToExpiry > 1 {
        return "exp. in \(daysToExpiry)d"
    }
    if daysToExpiry == 1 {
        return "exp. in 1d"
    }
    return "expired"
}

struct ContentView: View {
    @ObservedObject var appState: AppState
    @State private var selectedView = STABLE_VIEWS.first!
    @State private var webView = WebView()

    @EnvironmentObject private var appDelegate: AppDelegate

    @ObservedObject private var viewMode = ViewModeManager()

    // when this variable is set, force hide the toolbar and show "Obscura" for the navigation title
    // otherwise let macOS manage the state and let the navigation title be driven from the navigation view shown
    @State private var loginViewShown: Bool
    // set alongside above, want to hide the sidebar when navigation is not allowed
    @State private var splitViewVisibility: NavigationSplitViewVisibility

    init(appState: AppState) {
        self.appState = appState

        let forceHide = appState.status.accountId == nil || appState.status.inNewAccountFlow
        self.loginViewShown = forceHide
        self.splitViewVisibility = forceHide ? .detailOnly : .automatic
    }

    var body: some View {
        NavigationSplitView(
            columnVisibility: self.$splitViewVisibility,
            sidebar: {
                List(self.viewMode.getViews(), id: \.self, selection: self.$selectedView) { view in
                    let label = Label(view.name.capitalized, systemImage: view.systemImageName)
                        .listItemTint(Color("ObscuraOrange"))
                    // hide badge if we simply do not know if should be shown
                    // guard let vpnStatus = self.getVpnStatus() else { return }
                    if view.name == "account" && self.appState.accountDaysTillExpiry.expiringSoon() {
                        label
                            .badge(Text(getBadgeText(self.appState.accountDaysTillExpiry.days!))
                                .monospacedDigit()
                                .foregroundColor(self.appState.accountDaysTillExpiry.days! <= 3 ? .red : .yellow)
                                .bold()
                            )
                            // this has to be here, otherwise the label color is system accent default
                            .listItemTint(Color("ObscuraOrange"))

                    } else {
                        label
                    }
                }
                .environment(\.sidebarRowSize, .large)
                .navigationSplitViewColumnWidth(min: 150, ideal: 200)
            }, detail: {
                self.webView
                    .navigationTitle(self.loginViewShown ? "Obscura" : self.selectedView.name.capitalized)
            }
        )
        .onChange(of: self.selectedView) { view in
            // inform webUI to update navigation
            self.webView.navigateTo(view: view)
        }
        .onChange(of: self.appState.status) { status in
            if status.accountId == nil || status.inNewAccountFlow {
                self.loginViewShown = true
                self.splitViewVisibility = .detailOnly
            } else if self.loginViewShown {
                // If previously force closed pop it open.
                self.loginViewShown = false
                self.splitViewVisibility = .automatic
            }
        }
        // once we are targeting macOS 14+, we can use .toolbar(removing: .sidebarToggle) instead
        .toolbar(self.loginViewShown ? .hidden : .automatic)
        .onAppear {
            logger.log("Registering openUrlCallback with AppDelegate")
            self.appDelegate.openUrlCallback = { url in
                self.handleObscuraURL(url: url)
            }
        }
    }

    func handleObscuraURL(url: URL) {
        logger.info("Handling URL: \(url, privacy: .public)")

        // From: https://developer.apple.com/documentation/xcode/defining-a-custom-url-scheme-for-your-app#Handle-incoming-URLs
        guard let components = NSURLComponents(url: url, resolvingAgainstBaseURL: true) else {
            logger.error("Failed to parse URL into components")
            return
        }

        switch components.path {
        case .some("/open"):
            fullyOpenManagerWindow()
        case .some("/payment-succeeded"):
            fullyOpenManagerWindow() // Open the manager window first
            self.webView.handlePaymentSucceeded()
        case let unknownPath:
            logger.error("Unknown URL path: \(unknownPath, privacy: .public)")
            fullyOpenManagerWindow()
        }
    }
}

struct SidebarButton: View {
    var body: some View {
        Button(action: self.toggleSidebar, label: {
            Image(systemName: "sidebar.leading")
        })
    }

    private func toggleSidebar() {
        #if os(macOS)
            NSApp.keyWindow?.firstResponder?.tryToPerform(#selector(NSSplitViewController.toggleSidebar(_:)), with: nil)
        #endif
    }
}

class WebViewController: NSViewController, WKNavigationDelegate {
    func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        // Check if the navigation action is a form submission
        if navigationAction.navigationType == .linkActivated {
            if let url = navigationAction.request.url {
                NSWorkspace.shared.open(url)
                decisionHandler(.cancel)
            } else {
                decisionHandler(.allow)
            }
        } else {
            decisionHandler(.allow)
        }
    }
}

struct WebView: NSViewRepresentable {
    let webView: WKWebView
    let webViewDelegate: WebViewController

    init() {
        let webConfiguration = WKWebViewConfiguration()
        // webConfiguration.preferences.javaScriptEnabled = true
        let error_capture_script = WKUserScript(source: js_error_capture, injectionTime: .atDocumentStart, forMainFrameOnly: false)
        webConfiguration.userContentController.addUserScript(error_capture_script)
        let log_capture_script = WKUserScript(source: js_log_capture, injectionTime: .atDocumentStart, forMainFrameOnly: false)
        webConfiguration.userContentController.addUserScript(log_capture_script)

        // add bridges (command, console.error, console.log) between JS and Swift
        // CommandHandler defined in ScriptMessageHandlers
        webConfiguration.userContentController.addScriptMessageHandler(CommandHandler.shared, contentWorld: .page, name: "commandBridge")
        webConfiguration.userContentController.add(ErrorHandler.shared, name: "errorBridge")
        webConfiguration.userContentController.add(LogHandler.shared, name: "logBridge")

        // for React application
        webConfiguration.preferences.setValue(true, forKey: "allowFileAccessFromFileURLs")
        webConfiguration.setValue(true, forKey: "allowUniversalAccessFromFileURLs")

        let preferences = WKPreferences()
        // note that text selection is disabled using CSS
        preferences.isTextInteractionEnabled = true
        webConfiguration.preferences = preferences

        self.webView = WKWebView(frame: .zero, configuration: webConfiguration)
        self.webViewDelegate = WebViewController()
        self.webView.navigationDelegate = self.webViewDelegate

        #if LOAD_DEV_SERVER
            let urlRequest = URLRequest(url: URL(string: "http://localhost:1420/")!)
            self.webView.load(urlRequest)
        #elseif DEBUG
            let url = Bundle.main.url(forResource: "index", withExtension: "html", subdirectory: "webview_content")!
            self.webView.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
        #else
            // see the Prod Client scheme
            let url = Bundle.main.url(forResource: "index", withExtension: "html", subdirectory: "build")!
            self.webView.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
        #endif
    }

    func makeNSView(context: Context) -> WKWebView {
        return self.webView
    }

    // [required] refresh the view
    func updateNSView(_ webView: WKWebView, context: Context) {}

    func navigateTo(view: NavView) {
        self.webView.evaluateJavaScript(WebView.generateNavEventJS(viewName: view.name))
    }

    static func generateNavEventJS(viewName: String) -> String {
        // reuse the variable `__WK_WEBKIT_NAV_EVENT__`
        let jsDispatchNavUpdateStr = """
        __WEBKIT_NAV_EVENT__ = new CustomEvent("navUpdate", { detail: "\(viewName)" });
        window.dispatchEvent(__WEBKIT_NAV_EVENT__);
        """
        return jsDispatchNavUpdateStr
    }

    func handlePaymentSucceeded() {
        self.webView.evaluateJavaScript(WebView.generatePaymentSucceededEventJS())
    }

    static func generatePaymentSucceededEventJS() -> String {
        return """
            window.dispatchEvent(new CustomEvent("paymentSucceeded"))
        """
    }
}

let js_error_capture = #"""
window.onerror = (message, source, lineno, colno, error) => {
    window.webkit.messageHandlers.errorBridge.postMessage({
      message: message,
      source: source,
      lineno: lineno,
      colno: colno,
    });
};
window.onunhandledrejection = (event) => {
    console.error(`unhandled promise rejection: ${event.reason}`)
}
"""#

let js_log_capture = #"""
console.log = function(...args) { window.webkit.messageHandlers.logBridge.postMessage({ level: "log", message: JSON.stringify(args) })}
console.info = function(...args) { window.webkit.messageHandlers.logBridge.postMessage({ level: "info", message: JSON.stringify(args) })}
console.warn = function(...args) { window.webkit.messageHandlers.logBridge.postMessage({ level: "warn", message: JSON.stringify(args) })}
console.error = function(...args) { window.webkit.messageHandlers.logBridge.postMessage({ level: "error", message: JSON.stringify(args) })}
console.debug = function(...args) { window.webkit.messageHandlers.logBridge.postMessage({ level: "debug", message: JSON.stringify(args) })}
"""#

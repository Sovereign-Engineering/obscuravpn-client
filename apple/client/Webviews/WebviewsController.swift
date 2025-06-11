import OSLog
import SwiftUI
import WebKit

private let logger = Logger(
    subsystem: Bundle.main.bundleIdentifier!,
    category: "WebviewsController"
)

// This is the navigation for all web views within the app
class WebviewsController: NSObject, ObservableObject, WKNavigationDelegate {
    @Published var showModalWebview: Bool = false

    @Published var obscuraWebView: ObscuraUIWebView? = nil
    @Published var externalWebView: ExternalWebView? = nil

    @Published var tab: AppView = .connection

    let useExernalBrowserForPayments = true

    func initializeWebviews(appState: AppState) {
        self.obscuraWebView = ObscuraUIWebView(appState: appState)
        self.externalWebView = ExternalWebView(appState: appState)
    }

    func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        if webView == self.obscuraWebView {
            // Check if the navigation action is a form submission
            if navigationAction.navigationType == .linkActivated, let url = navigationAction.request.url {
                #if os(macOS)
                    NSWorkspace.shared.open(url)
                #else
                    self.handleWebsiteLinkiOS(url: url)
                #endif
                decisionHandler(.cancel)
            } else {
                decisionHandler(.allow)
            }
        } else {
            if let url = navigationAction.request.url, url.absoluteString.contains("obscuravpn") {
                self.handleObscuraURL(url: url)
            }
            decisionHandler(.allow)
        }
    }

    #if !os(macOS)
        private func handleWebsiteLinkiOS(url: URL) {
            // Check that it is a staging.obscura.net or obscura.net url
            guard
                let components = NSURLComponents(
                    url: url,
                    resolvingAgainstBaseURL: true
                ), let path = components.path, components.host?.contains("obscura") ?? false
            else {
                logger.error("Failed to parse URL into components")
                return
            }

            if (path.contains("pay") && self.useExernalBrowserForPayments) ||
                // "Check connection link"
                path.contains("check") ||
                // "Website" button
                path == "/"
            {
                UIApplication.shared.open(url)
                return
            }

            // Open modal browser
            Task { @MainActor in
                // Clear webview
                self.externalWebView?.webView.load(URLRequest(url: URL(string: "about:blank")!))

                // Load the requested page
                self.externalWebView?.webView.load(URLRequest(url: url))

                self.showModalWebview = true
            }
        }
    #endif

    func handleObscuraURL(url: URL) {
        logger.info("Handling URL: \(url, privacy: .public)")

        // From: https://developer.apple.com/documentation/xcode/defining-a-custom-url-scheme-for-your-app#Handle-incoming-URLs
        guard
            let components = NSURLComponents(
                url: url,
                resolvingAgainstBaseURL: true
            )
        else {
            logger.error("Failed to parse URL into components")
            return
        }

        #if os(macOS)
            fullyOpenManagerWindow()
        #else
            self.showModalWebview = false
        #endif

        switch components.path {
        case .some("/open"):
            break
        case .some("/payment-succeeded"):
            self.obscuraWebView?.handlePaymentSucceeded()
        case .some("/account"):
            self.tab = .account
        case let unknownPath:
            logger.error(
                "Unknown URL path: \(unknownPath, privacy: .public)"
            )
        }
    }
}

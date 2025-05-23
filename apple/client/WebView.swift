import SwiftUI
import WebKit

class WebViewController: UXViewController, WKNavigationDelegate {
    func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
        // Check if the navigation action is a form submission
        if navigationAction.navigationType == .linkActivated {
            if let url = navigationAction.request.url {
                #if os(macOS)
                    NSWorkspace.shared.open(url)
                #endif
                decisionHandler(.cancel)
            } else {
                decisionHandler(.allow)
            }
        } else {
            decisionHandler(.allow)
        }
    }
}

struct WebView: UXViewRepresentable {
    let webView: WKWebView
    let webViewDelegate: WebViewController

    init(appState: AppState) {
        let webConfiguration = WKWebViewConfiguration()
        // webConfiguration.preferences.javaScriptEnabled = true
        let error_capture_script = WKUserScript(source: js_error_capture, injectionTime: .atDocumentStart, forMainFrameOnly: false)
        webConfiguration.userContentController.addUserScript(error_capture_script)
        let log_capture_script = WKUserScript(source: js_log_capture, injectionTime: .atDocumentStart, forMainFrameOnly: false)
        webConfiguration.userContentController.addUserScript(log_capture_script)

        // add bridges (command, console.error, console.log) between JS and Swift
        webConfiguration.userContentController.addScriptMessageHandler(CommandHandler(appState: appState), contentWorld: .page, name: "commandBridge")
        webConfiguration.userContentController.add(ErrorHandler.shared, name: "errorBridge")
        webConfiguration.userContentController.add(LogHandler.shared, name: "logBridge")

        // for React application
        webConfiguration.setValue(true, forKey: "allowUniversalAccessFromFileURLs")
        webConfiguration.preferences.setValue(true, forKey: "allowFileAccessFromFileURLs")
        // note that text selection is disabled using CSS
        webConfiguration.preferences.isTextInteractionEnabled = true
        #if DEBUG
            webConfiguration.preferences.setValue(true, forKey: "developerExtrasEnabled")
        #endif
        self.webView = WKWebView(frame: .zero, configuration: webConfiguration)
        self.webViewDelegate = WebViewController()
        self.webView.navigationDelegate = self.webViewDelegate

        #if LOAD_DEV_SERVER
            let urlRequest = URLRequest(url: URL(string: "http://localhost:1420/")!)
            self.webView.load(urlRequest)
        #else
            // see the Prod Client scheme
            let url = Bundle.main.url(forResource: "index", withExtension: "html", subdirectory: "build")!
            self.webView.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
        #endif

        #if !os(macOS)
            // Safe area ignore
            // https://stackoverflow.com/a/47814446/3833632
            self.webView.scrollView.contentInsetAdjustmentBehavior = .never
        #endif
    }

    func navigateTo(view: AppView) {
        self.webView.evaluateJavaScript(WebView.generateNavEventJS(viewName: view.ipcValue))
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

    #if os(iOS)
        func makeCoordinator() -> Coordinator {
            Coordinator(self)
        }
    #endif
}

// MARK: - AppKit

extension WebView {
    func makeNSView(context: Context) -> WKWebView {
        return self.webView
    }

    // [required] refresh the view
    func updateNSView(_ webView: WKWebView, context: Context) {}
}

// MARK: - UIKit

#if os(iOS)

    extension WebView {
        func makeUIView(context: Context) -> WKWebView {
            self.webView.scrollView.delegate = context.coordinator
            return self.webView
        }

        func updateUIView(_ webView: WKWebView, context: Context) {}
    }

    class Coordinator: NSObject, UIScrollViewDelegate {
        var parent: WebView

        init(_ parent: WebView) {
            self.parent = parent
        }

        func scrollViewWillBeginZooming(_ scrollView: UIScrollView, with view: UIView?) {
            scrollView.pinchGestureRecognizer?.isEnabled = false
        }

        func scrollViewDidZoom(_ scrollView: UIScrollView) {
            scrollView.minimumZoomScale = scrollView.zoomScale
            scrollView.maximumZoomScale = scrollView.zoomScale
        }
    }

#endif

let js_error_capture = #"""
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
"""#

let js_log_capture = #"""
function log(type, msg, ...args) {
    let formatted = [type, msg, ...args.map(a => JSON.stringify(a, undefined, "\t"))].join(" ");
    window.webkit.messageHandlers.logBridge.postMessage(formatted);
}
console.debug = log.bind(null, "debug:");
console.log = log.bind(null, "log:");
console.warn = log.bind(null, "warn:");
console.error = log.bind(null, "error:");
"""#

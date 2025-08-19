import SwiftUI
import WebKit

class ObscuraUIWebView: WKWebView {
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
        super.init(frame: .zero, configuration: webConfiguration)
        self.navigationDelegate = appState.webviewsController

        #if LOAD_DEV_SERVER
            let urlRequest = URLRequest(url: URL(string: "http://localhost:1420/")!)
            self.load(urlRequest)
        #else
            // see the Prod Client scheme
            let url = Bundle.main.url(forResource: "index", withExtension: "html", subdirectory: "build")!
            self.loadFileURL(url, allowingReadAccessTo: url.deletingLastPathComponent())
        #endif

        #if !os(macOS)
            // Safe area ignore
            // https://stackoverflow.com/a/47814446/3833632
            self.scrollView.delegate = self
            self.scrollView.contentInsetAdjustmentBehavior = .never
        #endif
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    func navigateTo(view: AppView) {
        self.evaluateJavaScript(
            ObscuraUIWebView.generateNavEventJS(viewName: view.ipcValue)
        )
        #if !os(macOS)
            self.scrollView.bounces = view.needsScroll
        #endif
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
        self.evaluateJavaScript(ObscuraUIWebView.generatePaymentSucceededEventJS())
    }

    static func generatePaymentSucceededEventJS() -> String {
        return """
            window.dispatchEvent(new CustomEvent("paymentSucceeded"))
        """
    }

    func handleScreenshotDetected() {
        self.evaluateJavaScript(ObscuraUIWebView.generateScreenshotDetectedEventJS())
    }

    static func generateScreenshotDetectedEventJS() -> String {
        return """
            window.dispatchEvent(new CustomEvent("screenshotDetected"))
        """
    }
}

#if !os(macOS)
    extension ObscuraUIWebView: UIScrollViewDelegate {
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

import WebKit

struct ExternalWebView: UXViewRepresentable {
    let webView: WKWebView

    init(appState: AppState) {
        let webConfiguration = WKWebViewConfiguration()
        #if DEBUG
            webConfiguration.preferences.setValue(true, forKey: "developerExtrasEnabled")
        #endif
        self.webView = WKWebView(frame: .zero, configuration: webConfiguration)
        self.webView.navigationDelegate = appState.webviewsController
        #if DEBUG
            if #available(iOS 16.4, macOS 13.3, *) {
                self.webView.isInspectable = true
            }
        #endif
    }
}

// MARK: - AppKit

extension ExternalWebView {
    func makeNSView(context: Context) -> WKWebView {
        return self.webView
    }

    // [required] refresh the view
    func updateNSView(_ webView: WKWebView, context: Context) {}
}

// MARK: - UIKit

#if os(iOS)

    extension ExternalWebView {
        func makeUIView(context: Context) -> UIView {
            return self.webView
        }

        func updateUIView(_ uiView: UIView, context: Context) {}
    }

#endif

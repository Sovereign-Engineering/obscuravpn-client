import SwiftUI
import WebKit

struct ObscuraUIWebViewMacOSWrapper: View {
    let webView: ObscuraUIWebView

    init(webView: ObscuraUIWebView) {
        self.webView = webView
    }

    var body: some View {
        WebViewRepresentable(webView: self.webView)
    }
}

private struct WebViewRepresentable: NSViewRepresentable {
    let webView: ObscuraUIWebView

    func makeNSView(context: Context) -> WKWebView {
        return self.webView
    }

    func updateNSView(_ webView: WKWebView, context: Context) {
        // No updates needed
    }
}

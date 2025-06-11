import SwiftUI
import WebKit

struct ObscuraUIMacOSWrapper: UXViewRepresentable {
    let webView: ObscuraUIWebView

    init(webView: ObscuraUIWebView) {
        self.webView = webView
    }
}

// MARK: - AppKit

// Hack not needed on macOS as NavigationSplitView allows each tab to share the same SwiftUI view
extension ObscuraUIMacOSWrapper {
    func makeNSView(context: Context) -> WKWebView {
        return self.webView
    }

    // [required] refresh the view
    func updateNSView(_ webView: WKWebView, context: Context) {}
}

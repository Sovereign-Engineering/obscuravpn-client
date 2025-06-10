import SwiftUI
import WebKit

/*
 This is a hack to allow multiple UXViewRepresentable's to share the same UIView
 This is normally not possible as the view returned from makeNSView or makeUIView needs to be shared

 This is because a view may only have one parent. If a view is given a new parent it is no longer a child of its old parent.
 UXViewRepresentable under the hood adds the view you give it into the heiarchy assigning it a parent. This means if that view was shared it is removed from its previous parent.
 If you switch between multiple UXViewRepresentable's that share a view you confuse the system when you go back to a previous UXViewRepresentable.
 That UXViewRepresentable expects that the view you gave is already the child of its internal node. However it was removed. This is an error.

 This class hacks around this with a static property and a state tie in. This way we can manually remove and re-add this view when the surrounding state (tab) changes.

 !!! This hack is currently not necessary on macOS where NavigationSplitView is used which allows you to share a view between multiple tabs.
 */

struct ObscuraUIWebViewSwiftUIWrapper: UXViewRepresentable {
    let webView: ObscuraUIWebView
    var currentTab: AppView?
    let myTab: AppView?

    init(webView: ObscuraUIWebView, currentTab: AppView? = nil, myTab: AppView? = nil) {
        self.webView = webView
        self.currentTab = currentTab
        self.myTab = myTab
    }
}

// MARK: - AppKit

// Hack not needed on macOS as NavigationSplitView allows each tab to share the same SwiftUI view
extension ObscuraUIWebViewSwiftUIWrapper {
    func makeNSView(context: Context) -> WKWebView {
        return self.webView
    }

    // [required] refresh the view
    func updateNSView(_ webView: WKWebView, context: Context) {}
}

// MARK: - UIKit

#if os(iOS)

    extension ObscuraUIWebViewSwiftUIWrapper {
        private weak static var owner: UIView?

        func makeUIView(context: Context) -> UIView {
            return UIView()
        }

        // Called when SwiftUI state changes such as the binding
        func updateUIView(_ hostedView: UIView, context: Context) {
            let currentlyMyTab = self.currentTab == self.myTab
            let isDuplicateCall = ObscuraUIWebViewSwiftUIWrapper.owner == hostedView
            if currentlyMyTab, !isDuplicateCall {
                self.removeWebview(
                    from: ObscuraUIWebViewSwiftUIWrapper.owner,
                    reassign: hostedView
                )
            }
        }

        func removeWebview(from view: UIView?, reassign to: UIView) {
            // Remove all children from previous view
            view?.subviews.forEach {
                $0.removeConstraints($0.constraints)
                $0.removeFromSuperview()
            }

            to.addSubview(self.webView)
            self.webView.translatesAutoresizingMaskIntoConstraints = false
            NSLayoutConstraint.activate([
                to.leadingAnchor
                    .constraint(equalTo: self.webView.leadingAnchor),
                to.trailingAnchor
                    .constraint(equalTo: self.webView.trailingAnchor),
                to.topAnchor
                    .constraint(equalTo: self.webView.topAnchor),
                to.bottomAnchor
                    .constraint(equalTo: self.webView.bottomAnchor),
            ])

            ObscuraUIWebViewSwiftUIWrapper.owner = to
        }
    }

#endif

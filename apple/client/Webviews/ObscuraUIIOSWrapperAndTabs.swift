import Combine
import OrderedCollections
import SwiftUI
import UIKit
import WebKit

// In SwiftUI in iOS targeting a minimum SDK of 18.0 SwiftUI TabView
// requires each tab views view to be different. Sharing the same web view
// between them creates significant problems. Workarounds were tried
// going to just use UIKit.

class ObscuraUIIOSViewAndTabsViewController: UIViewController {
    private let webView: ObscuraUIWebView
    private let tabBar: UITabBar
    private let tabBarItems: [UITabBarItem]
    private let webviewsController: WebviewsController
    private let tabs: OrderedSet<AppView>

    var showTabBar: Bool {
        didSet {
            self.setupLayout()
        }
    }

    private var cancellables = Set<AnyCancellable>()

    init(
        webView: ObscuraUIWebView,
        webviewsController: WebviewsController,
        tabs: OrderedSet<AppView>,
        showTabBar: Bool
    ) {
        self.showTabBar = showTabBar
        self.webView = webView
        self.tabBar = UITabBar()
        self.webviewsController = webviewsController
        self.tabBarItems = tabs.map { view in
            let item = UITabBarItem(
                title: view.rawValue.capitalized,
                image: UIImage(systemName: view.systemImageName),
                selectedImage: UIImage(systemName: view.systemImageName)
            )
            return item
        }
        self.tabs = tabs

        super.init(nibName: nil, bundle: nil)

        self.setupTabBar()
        self.setupLayout()

        webviewsController.$tab.sink { [weak self] newTab in
            self?.navigateTo(view: newTab)
        }.store(in: &self.cancellables)
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) has not been implemented")
    }

    private func setupTabBar() {
        self.tabBar.items = self.tabBarItems
        self.tabBar.selectedItem = self.tabBarItems.first
        self.tabBar.delegate = self
        self.tabBar.tintColor = UIColor(named: "ObscuraOrange")
    }

    private func setupLayout() {
        // Remove constraints and views if they were already subviews
        self.webView.removeFromSuperview()
        self.tabBar.removeFromSuperview()

        view.addSubview(self.webView)
        self.webView.translatesAutoresizingMaskIntoConstraints = false

        if self.showTabBar {
            view.insertSubview(self.tabBar, aboveSubview: self.webView)
            self.tabBar.translatesAutoresizingMaskIntoConstraints = false

            NSLayoutConstraint.activate([
                self.webView.topAnchor.constraint(equalTo: view.topAnchor),
                self.webView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                self.webView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                self.webView.bottomAnchor.constraint(equalTo: self.tabBar.topAnchor),

                self.tabBar.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                self.tabBar.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                self.tabBar.bottomAnchor.constraint(equalTo: view.safeAreaLayoutGuide.bottomAnchor),
            ])
        } else {
            NSLayoutConstraint.activate([
                self.webView.topAnchor.constraint(equalTo: view.topAnchor),
                self.webView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
                self.webView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
                self.webView.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            ])
        }
    }

    private func navigateTo(view: AppView) {
        if let index = tabs.firstIndex(of: view) {
            self.tabBar.selectedItem = self.tabBarItems[index]
        }
        self.webView.navigateTo(view: view)
    }
}

// MARK: - UITabBarDelegate

extension ObscuraUIIOSViewAndTabsViewController: UITabBarDelegate {
    func tabBar(_ tabBar: UITabBar, didSelect item: UITabBarItem) {
        guard let index = tabBarItems.firstIndex(of: item), index < tabs.count else { return }

        let selectedView = self.tabs[index]
        self.webviewsController.tab = selectedView
    }
}

// MARK: - SwiftUI Wrapper

struct ObscuraUIIOSViewAndTabsWrapper: UIViewControllerRepresentable {
    let webView: ObscuraUIWebView
    let webviewsController: WebviewsController
    let tabs: OrderedSet<AppView>
    let showTabBar: Bool

    func makeUIViewController(context: Context) -> ObscuraUIIOSViewAndTabsViewController {
        return ObscuraUIIOSViewAndTabsViewController(
            webView: self.webView,
            webviewsController: self.webviewsController,
            tabs: self.tabs,
            showTabBar: self.showTabBar
        )
    }

    func updateUIViewController(_ uiViewController: ObscuraUIIOSViewAndTabsViewController, context: Context) {
        uiViewController.showTabBar = self.showTabBar
    }
}

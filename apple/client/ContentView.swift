import OrderedCollections
import OSLog
import SwiftUI
import UniformTypeIdentifiers
import WebKit

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "ContentView")

enum AppView: String, Hashable, Identifiable {
    case account
    case connection
    case location
    case settings
    case help
    case about
    case developer

    var id: String {
        self.rawValue
    }

    var systemImageName: String {
        switch self {
        case .account:
            "person.circle"
        case .connection:
            "network.badge.shield.half.filled"
        case .location:
            "mappin.and.ellipse"
        case .settings:
            "gear"
        case .help:
            "questionmark.circle"
        case .about:
            "info.circle"
        case .developer:
            "book.and.wrench"
        }
    }

    var ipcValue: String {
        self.rawValue
    }
}

let STABLE_VIEWS: OrderedSet<AppView> = OrderedSet([.connection, .location, .account, .settings, .help, .about])

let EXPERIMETNAL_VIEWS: OrderedSet<AppView> = OrderedSet()

let DEBUG_VIEWS: OrderedSet<AppView> = OrderedSet([.developer])

let VIEW_MODES = [
    STABLE_VIEWS,
    STABLE_VIEWS.union(DEBUG_VIEWS),
    STABLE_VIEWS.union(EXPERIMETNAL_VIEWS).union(DEBUG_VIEWS),
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
        #if os(macOS)
            self.eventMonitor = NSEvent.addLocalMonitorForEvents(matching: .keyDown) { event in
                if event.charactersIgnoringModifiers == "D", event.modifierFlags.contains(.command) {
                    // Cmd+Shift+d
                    self.viewIndex = (self.viewIndex + 1) % VIEW_MODES.count
                    return nil
                }
                return event
            }
        #endif
    }

    deinit {
        #if os(macOS)
            if self.eventMonitor != nil {
                NSEvent.removeMonitor(self.eventMonitor!)
            }
        #endif
    }

    func getViews() -> OrderedSet<AppView> {
        return VIEW_MODES[self.viewIndex]
    }

    func getIOSViews() -> OrderedSet<AppView> {
        let iOSViews: Set<AppView> = [.connection, .location, .account, .settings, .about]
        return self.getViews().filter { iOSViews.contains($0) }
    }
}

extension AccountStatus {
    var badgeText: String? {
        guard let days = daysUntilExpiry() else { return nil }
        if !expiringSoon() {
            return nil
        }
        if days > 3 {
            return "expires soon"
        }
        if days > 1 {
            return "exp. in \(days)d"
        }
        if days == 1 {
            return "exp. in 1d"
        }
        return isActive() ? "exp. today" : "expired"
    }

    var badgeColor: Color? {
        guard let days = daysUntilExpiry() else { return nil }
        return days <= 3 ? .red : .yellow
    }
}

struct ContentView: View {
    @ObservedObject var appState: AppState
    @State private var selectedView = STABLE_VIEWS.first!
    @State private var webView: WebView

    // when accountBadge and badgeColor are nil, the account status is either unknown OR a badge does not need to be shown
    // if ever the account is reset to nil, these variables will maintain their last computed values
    // see https://linear.app/soveng/issue/OBS-1159/ regarding why account could be reset to nil
    @State private var accountBadge: String?
    @State private var badgeColor: Color?
    @State private var indicateUpdateAvailable: Bool = false

    #if os(macOS)
        @EnvironmentObject private var appDelegate: AppDelegate
    #endif

    @ObservedObject private var viewMode = ViewModeManager()

    // when this variable is set, force hide the toolbar and show "Obscura" for the navigation title
    // otherwise let macOS manage the state and let the navigation title be driven from the navigation view shown
    @State private var loginViewShown: Bool
    // set alongside above, want to hide the sidebar when navigation is not allowed
    @State private var splitViewVisibility: NavigationSplitViewVisibility

    let accountBadgeTimer = Timer.publish(every: 5, on: .main, in: .common).autoconnect()

    init(appState: AppState) {
        self.appState = appState
        self.webView = WebView(appState: appState)
        let forceHide = appState.status.accountId == nil || appState.status.inNewAccountFlow
        self.loginViewShown = forceHide
        self.splitViewVisibility = forceHide ? .detailOnly : .automatic
    }

    var body: some View {
        self.content
            .onReceive(self.accountBadgeTimer, perform: { _ in
                if let account = self.appState.status.account {
                    self.accountBadge = account.badgeText
                    self.badgeColor = account.badgeColor
                }
                self.indicateUpdateAvailable = self.appState.osStatus.get().updaterStatus.type == .available
            })
            .onChange(of: self.selectedView) { view in
                // inform webUI to update navigation
                self.webView.navigateTo(view: view)
            }
            .onChange(of: self.appState.status) { status in
                if let account = self.appState.status.account {
                    self.accountBadge = account.badgeText
                    self.badgeColor = account.badgeColor
                }
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
                #if os(macOS)
                    self.appDelegate.openUrlCallback = { url in
                        self.handleObscuraURL(url: url)
                    }
                #endif
            }
    }

    @ViewBuilder func viewLabel(_ view: AppView) -> some View {
        let label = Label(view.rawValue.capitalized, systemImage: view.systemImageName)
            .listItemTint(Color("ObscuraOrange"))
        if view == .account && self.accountBadge != nil && self.badgeColor != nil {
            label.badge(Text(self.accountBadge!)
                .monospacedDigit()
                .foregroundColor(self.badgeColor)
                .bold()
            )
        } else if view == .account && self.indicateUpdateAvailable {
            HStack {
                label
                Spacer()
                Circle()
                    .fill(Color.green)
                    .frame(width: 8, height: 8)
            }
        } else {
            label
        }
    }

    var content: some View {
        #if os(macOS)
            NavigationSplitView(columnVisibility: self.$splitViewVisibility) {
                List(self.viewMode.getViews(), id: \.self, selection: self.$selectedView) { view in
                    self.viewLabel(view)
                }
                .environment(\.sidebarRowSize, .large)
                .navigationSplitViewColumnWidth(min: 175, ideal: 200)
            } detail: {
                self.webView
                    .navigationTitle(self.loginViewShown ? "Obscura" : self.selectedView.rawValue.capitalized)
            }
        #else
            VStack {
                self.webView
                    .ignoresSafeArea()
                if !self.loginViewShown {
                    HStack {
                        ForEach(self.viewMode.getIOSViews()) { view in
                            Button {
                                self.selectedView = view
                            } label: {
                                self.viewLabel(view)
                                    .frame(maxWidth: .infinity)
                            }
                        }
                    }
                    .frame(alignment: .bottom)
                }
            }
        #endif
    }

    func handleObscuraURL(url: URL) {
        logger.info("Handling URL: \(url, privacy: .public)")

        // From: https://developer.apple.com/documentation/xcode/defining-a-custom-url-scheme-for-your-app#Handle-incoming-URLs
        guard let components = NSURLComponents(url: url, resolvingAgainstBaseURL: true) else {
            logger.error("Failed to parse URL into components")
            return
        }

        #if os(macOS)
            switch components.path {
            case .some("/open"):
                fullyOpenManagerWindow()
            case .some("/payment-succeeded"):
                fullyOpenManagerWindow() // Open the manager window first
                self.webView.handlePaymentSucceeded()
            case .some("/account"):
                fullyOpenManagerWindow()
                self.selectedView = .account
            case let unknownPath:
                logger.error("Unknown URL path: \(unknownPath, privacy: .public)")
                fullyOpenManagerWindow()
            }
        #endif
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

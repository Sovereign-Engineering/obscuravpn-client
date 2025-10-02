import Combine
import OSLog
import SwiftUI
import UserNotifications

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "StatusMenu")
private let creatingDebuggingArchiveStr = "Creating Debugging Archive (takes a few minutes)"
private let createDebuggingArchiveStr = "Create Debugging Archive"

// https://multi.app/blog/pushing-the-limits-nsstatusitem
final class StatusItemManager: ObservableObject {
    private var hostingView: NSHostingView<StatusItem>
    private var statusItem: NSStatusItem

    private var debuggingMenuItem: NSMenuItem
    private var viewLatestDebugItem: NSMenuItem
    private var accountMenuItemSeparator: NSMenuItem
    private var accountMenuItem: NSMenuItem
    private var quickConnectMenuItem: NSMenuItem
    private var locationSubmenu: NSMenu

    private var sizePassthrough = PassthroughSubject<CGSize, Never>()
    private var bandwidthStatusModel = BandwidthStatusModel()
    private var osStatusModel = OsStatusModel()
    @Published private var cityNames: [CityExit: String] = [:]

    // ensures sink() closures are retained in memory
    // cancel() will be called on each item upon deinit
    private var cancellables = Set<AnyCancellable>()
    private var accountUpdateTask: Task<Void, Error>?

    // intentionally empty to ensure that the menu item can be highlighted
    @objc func emptyAction() {}

    init() {
        Self.exitRefreshSubscriber().store(in: &self.cancellables)

        self.statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        self.hostingView = NSHostingView(
            rootView: StatusItem(
                sizePassthrough: self.sizePassthrough,
                bandwidthStatusModel: self.bandwidthStatusModel,
                osStatusModel: self.osStatusModel
            ))
        self.hostingView.frame = NSRect(x: 0, y: 0, width: 100, height: 24)
        self.statusItem.button?.frame = self.hostingView.frame
        self.statusItem.button?.addSubview(self.hostingView)

        let menu = NSMenu()

        let toggleMenuItem = NSMenuItem(
            title: "Toggle VPN",
            action: #selector(self.emptyAction),
            keyEquivalent: ""
        )
        let toggleHostingView = MenuItemView(ObscuraToggle(osStatusModel: self.osStatusModel))
        // https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/MenuList/Articles/ViewsInMenuItems.html
        toggleMenuItem.view = toggleHostingView

        let locationSubmenuMenuItem = NSMenuItem()
        locationSubmenuMenuItem.title = "Connect via..."
        locationSubmenuMenuItem.image = NSImage(named: "custom.globe.badge.gearshape.fill")

        let showWindowMenuItem = NSMenuItem(
            title: "Open Obscura Manager...",
            action: #selector(self.showWindow),
            keyEquivalent: ""
        )
        let image = NSImage(named: NSImage.applicationIconName)!
        image.size = NSSize(width: 16.0, height: 16.0)
        showWindowMenuItem.image = image

        self.accountMenuItemSeparator = NSMenuItem.separator()
        self.accountMenuItemSeparator.isHidden = true

        self.accountMenuItem = NSMenuItem(title: "", action: #selector(self.emptyAction), keyEquivalent: "")
        self.accountMenuItem.isHidden = true

        let bandwidthStatusItem = NSMenuItem()
        bandwidthStatusItem.view = MenuItemView(BandwidthStatus(bandwidthStatusModel: self.bandwidthStatusModel))

        self.debuggingMenuItem = NSMenuItem(
            title: createDebuggingArchiveStr,
            action: #selector(self.createDebuggingArchiveAction),
            keyEquivalent: ""
        )

        self.viewLatestDebugItem = NSMenuItem(
            title: "View Latest Debug Archive",
            action: #selector(self.viewLatestDebugArchive),
            keyEquivalent: ""
        )
        self.viewLatestDebugItem.isHidden = true

        self.locationSubmenu = NSMenu()
        locationSubmenuMenuItem.submenu = self.locationSubmenu

        self.quickConnectMenuItem = NSMenuItem(
            title: "Quick Connect",
            action: #selector(self.connectAction),
            keyEquivalent: ""
        )
        self.quickConnectMenuItem.representedObject = ExitSelector.any
        self.quickConnectMenuItem.indentationLevel = 1
        self.locationSubmenu.addItem(self.quickConnectMenuItem)

        let loadingLocationsItem = NSMenuItem(
            title: "Loading Locations...",
            action: nil,
            keyEquivalent: ""
        )
        loadingLocationsItem.indentationLevel = 1
        self.locationSubmenu.addItem(loadingLocationsItem)

        self.addMoreLocationsItem()

        toggleMenuItem.target = self
        showWindowMenuItem.target = self
        self.quickConnectMenuItem.target = self
        self.accountMenuItem.target = self
        self.debuggingMenuItem.target = self
        self.viewLatestDebugItem.target = self

        let disconnectAndQuitItem = NSMenuItem(
            title: "Quit and Disconnect", action: #selector(self.disconnectAndQuit),
            keyEquivalent: ""
        )
        disconnectAndQuitItem.target = self

        menu.items = [
            toggleMenuItem,
            locationSubmenuMenuItem,
            .separator(),
            showWindowMenuItem,
            self.accountMenuItemSeparator,
            self.accountMenuItem,
            .separator(),
            Self.createSectionHeaderMenuItem(title: "Live Usage"),
            bandwidthStatusItem,
            .separator(),
            self.debuggingMenuItem,
            self.viewLatestDebugItem,
            .init(title: sourceVersion(), action: nil, keyEquivalent: ""),
            disconnectAndQuitItem,
        ]

        self.statusItem.menu = menu

        self.sizePassthrough.sink { [weak self] size in
            let frame = NSRect(origin: .zero, size: .init(width: size.width, height: 24))
            self?.hostingView.frame = frame
            self?.statusItem.button?.frame = frame
        }.store(in: &self.cancellables)

        Publishers.CombineLatest(self.$cityNames,
                                 StartupModel.shared.$appState
                                     .filter { $0 != nil }
                                     .flatMap { $0!.$status }).sink { [weak self] _, newStatus in
            self?.triggerSetLocationMenuItems()
        }.store(in: &self.cancellables)

        StartupModel.shared.$appState
            .compactMap { $0 }
            .first()
            .sink { appState in
                Task { [weak self] in
                    var exitListKnownVersion: String?
                    while true {
                        guard let self = self else { return }
                        do {
                            let result = try await getCityNames(appState.manager, knownVersion: exitListKnownVersion)
                            exitListKnownVersion = result.version
                            self.cityNames = result.cityNames
                        } catch {
                            logger.error("Failed to get exit list: \(error, privacy: .public)")
                            try await Task.sleep(seconds: 1)
                        }
                    }
                }

                appState.$status
                    .map { $0.account }
                    .removeDuplicates()
                    .sink { [weak self] _ in
                        self?.accountUpdateTask?.cancel()

                        self?.accountUpdateTask = Task { [weak self] in
                            while true {
                                self?.updateAccountItem()
                                if let account = appState.status.account {
                                    if !account.isActive() {
                                        try await Task.sleep(for: .seconds(30), tolerance: .seconds(10))
                                    } else if account.expiringSoon() {
                                        try await Task.sleep(for: .seconds(60), tolerance: .seconds(30))
                                    } else {
                                        // sleep until we expect account item to show up
                                        let toppedUpExpirationDate = account.accountInfo.topUp?.creditExpiresAt ?? 0
                                        let stripeEndDate = account.accountInfo.stripeSubscription?.currentPeriodEnd ?? 0
                                        let appleEndDate = account.accountInfo.appleSubscription?.renewalTime ?? 0
                                        let end = max(toppedUpExpirationDate, stripeEndDate, appleEndDate, 0)

                                        // 60 seconds after threshold (-10 days) timestamp
                                        let sleepUntilTime = end - 10 * 24 * 60 * 60 + 60
                                        let sleepUntilDate = Date(timeIntervalSince1970: TimeInterval(sleepUntilTime))

                                        let sleepInterval = sleepUntilDate.timeIntervalSinceNow
                                        if sleepInterval > 0 {
                                            try await Task.sleep(for: .seconds(sleepInterval), tolerance: .seconds(30))
                                        } else {
                                            logger.error("account is not expiring soon, yet the estimated recheck date is in the past")
                                            try await Task.sleep(for: .seconds(60), tolerance: .seconds(30))
                                        }
                                    }
                                } else {
                                    try await Task.sleep(for: .seconds(30), tolerance: .seconds(30))
                                }
                            }
                        }
                    }.store(in: &self.cancellables)

                // MainActor since osStatusModel is used by layout engine
                Task { @MainActor [weak self] in
                    while true {
                        guard let self = self else { return }
                        self.osStatusModel.osStatus = await appState.getOsStatus(knownVersion: self.osStatusModel.osStatus?.version)
                    }
                }

                // MainActor since bandwidth status is used by layout engine
                Task { @MainActor [weak self] in
                    var trafficStats: TrafficStats?
                    do {
                        while true {
                            try await Task.sleep(seconds: 1)
                            if case .connected = appState.status.vpnStatus {
                                do {
                                    let newTrafficStats = try await appState.getTrafficStats()
                                    let oldTrafficStats = trafficStats
                                    trafficStats = newTrafficStats
                                    if let oldTrafficStats = oldTrafficStats, oldTrafficStats.connId == newTrafficStats.connId {
                                        let (txBytesDelta, overflowedTx) = newTrafficStats.txBytes.subtractingReportingOverflow(oldTrafficStats.txBytes)
                                        let (rxBytesDelta, overflowedRx) = newTrafficStats.rxBytes.subtractingReportingOverflow(oldTrafficStats.rxBytes)
                                        let (msElapsed, overflowedT) = newTrafficStats.connectedMs.subtractingReportingOverflow(oldTrafficStats.connectedMs)
                                        if overflowedTx || overflowedRx || overflowedT {
                                            logger.info("oldTrafficStats: tx \(oldTrafficStats.txBytes, privacy: .public), rx \(oldTrafficStats.rxBytes, privacy: .public), timestamp \(oldTrafficStats.connectedMs, privacy: .public)")
                                            logger.info("newTrafficStats: tx \(newTrafficStats.txBytes, privacy: .public), rx \(newTrafficStats.rxBytes, privacy: .public), timestamp \(newTrafficStats.connectedMs, privacy: .public)")
                                            #if DEBUG
                                                fatalError("unexpected overflowed in bandwidth subtractions. tx overflowed? \(overflowedTx), rx overflowed? \(overflowedRx), timestamp overflowed?  \(overflowedT)")
                                            #else
                                                logger.error("unexpected overflowed in bandwidth subtractions. tx overflowed? \(overflowedTx, privacy: .public), rx overflowed? \(overflowedRx, privacy: .public), timestamp overflowed?  \(overflowedT, privacy: .public)")
                                            #endif
                                        } else {
                                            let secondsDelta = Double(msElapsed) / 1000
                                            if secondsDelta > 0 {
                                                self?.bandwidthStatusModel.uploadBandwidth = BandwidthFmt.fromTransferRate(bytesPerSecond: Double(txBytesDelta) / secondsDelta)
                                                self?.bandwidthStatusModel.downloadBandwidth = BandwidthFmt.fromTransferRate(bytesPerSecond: Double(rxBytesDelta) / secondsDelta)
                                                continue
                                            }
                                        }
                                    }
                                } catch {
                                    logger.info("StatusItemManager getTrafficStats failed while connected \(error, privacy: .public)")
                                    continue
                                }
                            }
                            self?.bandwidthStatusModel.uploadBandwidth = BandwidthFmt.fromTransferRate(
                                bytesPerSecond: 0)
                            self?.bandwidthStatusModel.downloadBandwidth = BandwidthFmt.fromTransferRate(
                                bytesPerSecond: 0)
                        }
                    }
                }
            }.store(in: &self.cancellables)

        self.osStatusModel.$osStatus.sink { [weak self] _ in
            self?.updateDebugBundleMenuItem()
        }.store(in: &self.cancellables)
    }

    @objc func connectAction(_ sender: NSMenuItem) {
        // app crashes if this function is async
        guard let exitSelector = sender.representedObject as? ExitSelector else {
            logger.error("connectAction called with incorrect sender.representedObject")
            return
        }
        Task {
            do {
                guard let appState = StartupModel.shared.appState else { return }
                try await appState.enableTunnel(TunnelArgs(exit: exitSelector))
            } catch {
                logger.error("Failed to connect from status location submenu: \(error, privacy: .public)")
            }
        }
    }

    @objc func showWindow() {
        fullyOpenManagerWindow()
    }

    @objc func openMoreLocations() {
        NSWorkspace.shared.open(URLs.AppLocationPage)
    }

    @objc func disconnectAndQuit() {
        StartupModel.shared.appState?.disableTunnel()
        NSApp.terminate(nil)
    }

    @objc func viewLatestDebugArchive() {
        if let mostRecentPath = self.osStatusModel.osStatus?.debugBundleStatus.latestPath {
            NSWorkspace.shared.selectFile(mostRecentPath, inFileViewerRootedAtPath: "")
        }
    }

    @objc func createDebuggingArchiveAction() {
        DispatchQueue.main.async {
            self.debuggingMenuItem.target = nil
            self.debuggingMenuItem.title = creatingDebuggingArchiveStr
        }
        Task {
            do {
                let _ = try await createDebuggingArchive(appState: StartupModel.shared.appState)
            } catch {
                logger.error("Error creating debug bundle: \(error, privacy: .public)")

                let content = UNMutableNotificationContent()
                content.title = "Error Creating Debug Bundle"
                content.body = error.localizedDescription
                content.interruptionLevel = .active
                content.sound = UNNotificationSound.default
                displayNotification(.debuggingBundleFailed, content)
            }
        }
    }

    private func getCityDisplayName(countryCode: String, cityCode: String) -> String {
        return self.cityNames[CityExit(city_code: cityCode, country_code: countryCode)] ?? cityCode
    }

    private func updateDebugBundleMenuItem() {
        DispatchQueue.main.async {
            if let debugBundleStatus = self.osStatusModel.osStatus?.debugBundleStatus {
                if debugBundleStatus.inProgress {
                    self.debuggingMenuItem.target = nil
                    self.debuggingMenuItem.title = creatingDebuggingArchiveStr
                    self.viewLatestDebugItem.isHidden = true
                } else if self.debuggingMenuItem.target == nil {
                    self.debuggingMenuItem.target = self
                    self.debuggingMenuItem.title = createDebuggingArchiveStr
                    let viewLatestAllowed = debugBundleStatus.latestPath != nil
                    self.viewLatestDebugItem.isHidden = !viewLatestAllowed
                }
            }
        }
    }

    private func triggerSetLocationMenuItems() {
        DispatchQueue.main.async {
            // Remove all items except the Quick Connect item (which is always first)
            self.locationSubmenu.items.removeLast(max(self.locationSubmenu.numberOfItems - 1, 0))

            if let appState = StartupModel.shared.appState {
                let pinnedLocations = appState.status.pinnedLocations
                let lastExit = appState.status.lastExit

                switch lastExit {
                case .any:
                    self.quickConnectMenuItem.state = .on
                default:
                    self.quickConnectMenuItem.state = .off
                }

                var lastExitIsPinned = false

                if !pinnedLocations.isEmpty {
                    let pinnedLocationsSubHeaderItem = Self.createSectionHeaderMenuItem(title: "Pinned Locations")
                    pinnedLocationsSubHeaderItem.indentationLevel = 1
                    self.locationSubmenu.addItem(pinnedLocationsSubHeaderItem)

                    for pinnedLocation in pinnedLocations {
                        // Do not show location in status menu if the pinned exit is not found in the fetched cityNames
                        let cityExit = CityExit(
                            city_code: pinnedLocation.city_code,
                            country_code: pinnedLocation.country_code
                        )
                        if !self.cityNames.isEmpty && self.cityNames[cityExit] == nil {
                            continue
                        }

                        let cityName = self.getCityDisplayName(
                            countryCode: pinnedLocation.country_code,
                            cityCode: pinnedLocation.city_code
                        )

                        let menuItem = NSMenuItem(
                            title: "\(cityName), \(pinnedLocation.country_code.uppercased())",
                            action: #selector(self.connectAction),
                            keyEquivalent: ""
                        )
                        menuItem.target = self
                        menuItem.representedObject = ExitSelector.city(
                            country_code: pinnedLocation.country_code,
                            city_code: pinnedLocation.city_code
                        )

                        // Check if this pinned location matches the last chosen exit
                        switch lastExit {
                        case .city(let country_code, let city_code):
                            if country_code == pinnedLocation.country_code && city_code == pinnedLocation.city_code {
                                menuItem.state = .on
                                lastExitIsPinned = true
                            }
                        default:
                            break
                        }

                        menuItem.indentationLevel = 1
                        self.locationSubmenu.addItem(menuItem)
                    }
                }

                // If the last chosen exit is a city that's not in the pinned locations, add a header and menu item
                if case .city(let country_code, let city_code) = lastExit, !lastExitIsPinned {
                    let nonPinnedLocationHeaderItem = Self.createSectionHeaderMenuItem(title: "Current Selection")
                    nonPinnedLocationHeaderItem.indentationLevel = 1
                    self.locationSubmenu.addItem(nonPinnedLocationHeaderItem)

                    let cityName = self.getCityDisplayName(
                        countryCode: country_code,
                        cityCode: city_code
                    )

                    let nonPinnedMenuItem = NSMenuItem(
                        title: "\(cityName), \(country_code.uppercased())",
                        action: #selector(self.connectAction),
                        keyEquivalent: ""
                    )
                    nonPinnedMenuItem.target = self
                    nonPinnedMenuItem.representedObject = ExitSelector.city(
                        country_code: country_code,
                        city_code: city_code
                    )
                    nonPinnedMenuItem.state = .on
                    nonPinnedMenuItem.indentationLevel = 1

                    self.locationSubmenu.addItem(nonPinnedMenuItem)
                }
            }
            self.addMoreLocationsItem()
        }
    }

    private static func createSectionHeaderMenuItem(title: String) -> NSMenuItem {
        if #available(macOS 14.0, *) {
            return NSMenuItem.sectionHeader(title: title)
        } else {
            return NSMenuItem(title: title, action: nil, keyEquivalent: "")
        }
    }

    private func addMoreLocationsItem() {
        self.locationSubmenu.addItem(NSMenuItem.separator())
        let moreLocationsMenuItem = NSMenuItem(
            title: "More Locations…",
            action: #selector(self.openMoreLocations),
            keyEquivalent: ""
        )
        moreLocationsMenuItem.target = self
        let image = NSImage(named: NSImage.applicationIconName)!
        image.size = NSSize(width: 16.0, height: 16.0)
        moreLocationsMenuItem.image = image
        self.locationSubmenu.addItem(moreLocationsMenuItem)
    }

    private static func refreshExitListIfNeeded() {
        Task {
            if let appState = StartupModel.shared.appState {
                do {
                    _ = try await refreshExitList(appState.manager, freshness: 3600)
                } catch {
                    logger.error(
                        "Failed to refresh exit list in status menu: \(error, privacy: .public)")
                }
            }
        }
    }

    private func updateAccountItem() {
        guard let appState = StartupModel.shared.appState else { return }
        if let account = appState.status.account {
            let secondsStamp = UInt64(Date().timeIntervalSince1970)
            var pollAccount = false
            if (!account.isActive() || account.daysUntilExpiry() == 0)
                && secondsStamp - account.lastUpdatedSec > 60 * 5
            {
                pollAccount = true
            } else if account.expiringSoon()
                && secondsStamp - account.lastUpdatedSec > 60 * 60 * 12
            {
                pollAccount = true
            }

            if pollAccount {
                Task {
                    // updateAccountItem task will restart upon appState.status.account change
                    try? await getAccountInfo(appState.manager)
                }
                return
            }

            DispatchQueue.main.async {
                let accountHostingView = MenuItemView(StatusItemAccount(account: account))
                self.accountMenuItem.view = accountHostingView
                self.accountMenuItem.isHidden = !account.expiringSoon() || appState.status.inNewAccountFlow
                self.accountMenuItemSeparator.isHidden = self.accountMenuItem.isHidden
            }
        } else {
            DispatchQueue.main.async {
                self.accountMenuItem.isHidden = true
                self.accountMenuItemSeparator.isHidden = true
            }
        }
    }

    private static func exitRefreshSubscriber() -> AnyCancellable {
        self.refreshExitListIfNeeded()
        return Timer.publish(every: 3660, tolerance: 60, on: .current, in: .common)
            .autoconnect()
            .sink { _ in
                Self.refreshExitListIfNeeded()
            }
    }
}

private struct SizePreferenceKey: PreferenceKey {
    static var defaultValue: CGSize = .zero
    static func reduce(value: inout CGSize, nextValue: () -> CGSize) { value = nextValue() }
}

struct StatusItem: View {
    var sizePassthrough: PassthroughSubject<CGSize, Never>
    @State private var osStatus: OsStatus?
    @ObservedObject var startupModel = StartupModel.shared
    @ObservedObject var bandwidthStatusModel: BandwidthStatusModel
    @ObservedObject var osStatusModel: OsStatusModel

    let connectingImageNames = ["MenuBarConnecting-1", "MenuBarConnecting-2", "MenuBarConnecting-3"]
    @State private var menuBarImage = "MenuBarDisconnected"
    @State private var statusIconIdx = 0
    let statusIconTimer = Timer.publish(every: 0.5, on: .main, in: .common).autoconnect()

    func getVpnStatus() -> NeVpnStatus? {
        return self.startupModel.appState?.status.vpnStatus
    }

    @ViewBuilder
    var mainContent: some View {
        HStack(spacing: 10) {
            HStack(spacing: 3) {
                ZStack {
                    Image(self.menuBarImage)
                        .renderingMode(.template)
                        .onReceive(self.statusIconTimer, perform: { _ in
                            if self.osStatusModel.osStatus?.osVpnStatus == .disconnecting {
                                self.menuBarImage = self.connectingImageNames[self.statusIconIdx]
                                // add a full count before using modulo to avoid negative indices
                                self.statusIconIdx = (self.statusIconIdx + self.connectingImageNames.count - 1)
                                    % self.connectingImageNames.count
                                return
                            }
                            switch self.getVpnStatus() {
                            case .connecting:
                                self.menuBarImage = self.connectingImageNames[self.statusIconIdx]
                                self.statusIconIdx = (self.statusIconIdx + 1) % self.connectingImageNames.count
                            case .connected:
                                self.menuBarImage = "MenuBarConnected"
                                if self.bandwidthStatusModel.uploadBandwidth.Intensity > 0 {
                                    self.menuBarImage += "Up"
                                }
                                if self.bandwidthStatusModel.downloadBandwidth.Intensity > 0 {
                                    self.menuBarImage += "Down"
                                }
                                self.statusIconIdx = self.connectingImageNames.count - 1
                            case .disconnected, nil:
                                self.menuBarImage = "MenuBarDisconnected"
                                self.statusIconIdx = 0
                            }
                        })
                    if self.menuBarImage.starts(with: "MenuBarConnected") {
                        Rectangle()
                            .frame(width: 4, height: 4)
                            .position(x: 20.5, y: 17)
                            .foregroundStyle(Color(red: 84 / 255, green: 214 / 255, blue: 97 / 255))
                    }
                }
            }
        }
        .padding(4)
        .padding(.bottom, 2)
        .fixedSize()
    }

    var body: some View {
        self.mainContent
            .overlay(
                GeometryReader { geometryProxy in
                    Color.clear
                        .preference(key: SizePreferenceKey.self, value: geometryProxy.size)
                }
            )
            .onPreferenceChange(
                SizePreferenceKey.self,
                perform: { size in
                    self.sizePassthrough.send(size)
                }
            )
    }
}

class OsStatusModel: ObservableObject {
    @Published var osStatus: OsStatus? = nil
}

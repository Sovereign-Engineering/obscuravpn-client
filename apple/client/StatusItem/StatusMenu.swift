import Combine
import OSLog
import SwiftUI
import UserNotifications

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "StatusMenu")

func getAccountStatusItemText(_ account: AccountStatus?) -> String? {
    guard let account = account else { return nil }
    guard let days = account.daysTillExpiry else { return nil }
    if !account.expiringSoon() {
        return nil
    }
    if days > 3 {
        return "Account expires soon"
    }
    if days > 1 {
        return "Account expires in \(days) days"
    }
    if days == 1 {
        return "Accounts expires in in 1 day"
    }
    return account.accountInfo.active ? "Account expires today" : "Account is expired"
}

// https://multi.app/blog/pushing-the-limits-nsstatusitem
final class StatusItemManager: ObservableObject {
    private var hostingView: NSHostingView<StatusItem>?
    private var statusItem: NSStatusItem?
    private var debuggingMenuItem: NSMenuItem?
    private var accountMenuItem: NSMenuItem?

    private var sizePassthrough = PassthroughSubject<CGSize, Never>()
    private var sizeCancellable: AnyCancellable?
    private var bandwidthStatusModel = BandwidthStatusModel()

    // intentionally empty to ensure that the menu item can be hightlighted
    @objc func toggleAction() {}

    func createStatusItem() {
        let statusItem: NSStatusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        let hostingView = NSHostingView(rootView: StatusItem(sizePassthrough: sizePassthrough, bandwidthStatusModel: bandwidthStatusModel))
        hostingView.frame = NSRect(x: 0, y: 0, width: 80, height: 24)
        statusItem.button?.frame = hostingView.frame
        statusItem.button?.addSubview(hostingView)

        let menu = NSMenu()

        let toggleMenuItem = NSMenuItem(title: "Toggle VPN", action: #selector(self.toggleAction), keyEquivalent: "")
        let toggleHostingView = MenuItemView(ObscuraToggle())
        // https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/MenuList/Articles/ViewsInMenuItems.html
        toggleMenuItem.view = toggleHostingView
        toggleMenuItem.target = self
        menu.addItem(toggleMenuItem)

        let showWindowMenuItem = NSMenuItem(title: "Open Obscura Manager...", action: #selector(self.showWindow), keyEquivalent: "")
        showWindowMenuItem.target = self
        let image = NSImage(named: NSImage.applicationIconName)!
        image.size = NSSize(width: 16.0, height: 16.0)
        showWindowMenuItem.image = image
        menu.addItem(showWindowMenuItem)

        self.accountMenuItem = NSMenuItem(title: "", action: nil, keyEquivalent: "")
        self.accountMenuItem!.isHidden = true
        menu.addItem(self.accountMenuItem!)

        Task { @MainActor in
            while true {
                if let appState = StartupModel.shared.appState {
                    self.accountMenuItem!.title = getAccountStatusItemText(appState.status.account) ?? ""
                    if let lastUpdatedSec = appState.status.account?.lastUpdatedSec {
                        let secondsStamp = UInt64(Date().timeIntervalSince1970)
                        if let account = appState.status.account {
                            var pollAccount = false
                            if (!account.accountInfo.active || account.daysTillExpiry == 0) && secondsStamp - account.lastUpdatedSec > 60 * 5 {
                                pollAccount = true
                            } else if account.expiringSoon() && secondsStamp - account.lastUpdatedSec > 60 * 60 * 12 {
                                pollAccount = true
                            }
                            if pollAccount {
                                _ = try? await getAccountInfo(appState.manager)
                            }
                        }
                    }
                } else {
                    self.accountMenuItem!.title = ""
                }
                self.accountMenuItem!.isHidden = self.accountMenuItem!.title.isEmpty
                do {
                    try await Task.sleep(seconds: 10)
                } catch {
                    return
                }
            }
        }

        menu.addItem(NSMenuItem.separator())
        if #available(macOS 14.0, *) {
            menu.addItem(NSMenuItem.sectionHeader(title: "Bandwidth Status"))
        } else {
            // fallback on earlier versions
            let bandwidthStatusTitleItem = NSMenuItem(title: "Bandwidth Status", action: nil, keyEquivalent: "")
            bandwidthStatusTitleItem.isEnabled = false
            menu.addItem(bandwidthStatusTitleItem)
        }
        let bandwidthStatusItem = NSMenuItem(title: "", action: nil, keyEquivalent: "")
        bandwidthStatusItem.view = MenuItemView(BandwidthStatus(bandwidthStatusModel: self.bandwidthStatusModel))
        menu.addItem(bandwidthStatusItem)

        menu.addItem(NSMenuItem.separator())

        self.debuggingMenuItem = NSMenuItem(title: "Create Debugging Archive", action: #selector(self.createDebuggingArchiveAction), keyEquivalent: "")
        self.debuggingMenuItem!.target = self
        menu.addItem(self.debuggingMenuItem!)

        menu.addItem(NSMenuItem(title: sourceVersion(), action: nil, keyEquivalent: ""))

        let disconnectAndQuitItem = NSMenuItem(title: "Quit and Disconnect", action: #selector(self.disconnectAndQuit), keyEquivalent: "")
        disconnectAndQuitItem.target = self
        menu.addItem(disconnectAndQuitItem)

        statusItem.menu = menu

        self.statusItem = statusItem
        self.hostingView = hostingView

        self.sizeCancellable = self.sizePassthrough.sink { [weak self] size in
            let frame = NSRect(origin: .zero, size: .init(width: size.width, height: 24))
            self?.hostingView?.frame = frame
            self?.statusItem?.button?.frame = frame
        }
    }

    @objc func showWindow() {
        fullyOpenManagerWindow()
    }

    @objc func disconnectAndQuit() {
        StartupModel.shared.appState?.disableTunnel()
        NSApp.terminate(nil)
    }

    @objc func createDebuggingArchiveAction() {
        Task {
            self.debuggingMenuItem!.isEnabled = false
            self.debuggingMenuItem?.title = "Creating Debugging Archive (takes a few minutes)"
            defer {
                self.debuggingMenuItem!.isEnabled = true
                self.debuggingMenuItem?.title = "Create Debugging Archive"
            }

            do {
                try await createDebuggingArchive()
            } catch {
                logger.error("Error creating debug bundle: \(error, privacy: .public)")

                let content = UNMutableNotificationContent()
                content.title = "Error Creating Debug Bundle"
                content.body = error.localizedDescription
                content.interruptionLevel = .active
                content.sound = UNNotificationSound.default
                displayNotification(
                    UNNotificationRequest(
                        identifier: "obscura-debugging-bundle-failed",
                        content: content,
                        trigger: nil
                    )
                )
            }
        }
    }
}

private struct SizePreferenceKey: PreferenceKey {
    static var defaultValue: CGSize = .zero
    static func reduce(value: inout CGSize, nextValue: () -> CGSize) { value = nextValue() }
}

let BANDWIDTH_MAX_THRESHOLD: Int = 250_000_000
let BANDWIDTH_MAX_INTENSITY: Int = 4 // levels

struct StatusItem: View {
    var sizePassthrough: PassthroughSubject<CGSize, Never>
    @State private var showWave: Bool = false
    @State private var menuShown: Bool = false
    @ObservedObject var startupModel = StartupModel.shared

    @ObservedObject var bandwidthStatusModel: BandwidthStatusModel

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
                            switch self.getVpnStatus() {
                            case .connecting, .reconnecting:
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
        .task {
            while true {
                do {
                    try await Task.sleep(seconds: 1)
                } catch {
                    logger.info("Task cancelled \(error, privacy: .public)")
                    return // Another task will be started.
                }
                do {
                    let newTrafficStats = try await startupModel.appState?.getTrafficStats()
                    guard let newTrafficStats = newTrafficStats else {
                        logger.info("getTrafficStats return nil")
                        continue
                    }
                    guard let oldTrafficStats = self.bandwidthStatusModel.trafficStats else {
                        self.bandwidthStatusModel.trafficStats = newTrafficStats
                        // no need to reset upload and download state as this is the first loop
                        continue
                    }
                    if oldTrafficStats.connId == newTrafficStats.connId {
                        // the connId check avoids arithmetic overflows when the vpn is disabled
                        let (txBytesDelta, overflowedTx) = newTrafficStats.txBytes.subtractingReportingOverflow(oldTrafficStats.txBytes)
                        let (rxBytesDelta, overflowedRx) = newTrafficStats.rxBytes.subtractingReportingOverflow(oldTrafficStats.rxBytes)
                        let (msElapsed, overflowedT) = newTrafficStats.timestampMs.subtractingReportingOverflow(oldTrafficStats.timestampMs)
                        if overflowedTx || overflowedRx || overflowedT {
                            logger.info("oldTrafficStats: tx \(oldTrafficStats.txBytes), rx \(oldTrafficStats.rxBytes), timestamp \(oldTrafficStats.timestampMs)")
                            logger.info("newTrafficStats: tx \(newTrafficStats.txBytes), rx \(newTrafficStats.rxBytes), timestamp \(newTrafficStats.timestampMs)")
                            #if DEBUG
                                fatalError("unexpected overflowed in bandwidth substractions. tx overflowed? \(overflowedTx), rx overflowed? \(overflowedRx), timestamp overflowed?  \(overflowedT)")
                            #else
                                logger.error("unexpected overflowed in bandwidth substractions. tx overflowed? \(overflowedTx), rx overflowed? \(overflowedRx), timestamp overflowed?  \(overflowedT)")
                            #endif
                        } else {
                            let secondsDelta = Double(msElapsed) / 1000
                            if secondsDelta > 0 {
                                self.bandwidthStatusModel.uploadBandwidth = BandwidthFmt.fromTransferRate(bytesPerSecond: Double(txBytesDelta) / secondsDelta)
                                self.bandwidthStatusModel.downloadBandwidth = BandwidthFmt.fromTransferRate(bytesPerSecond: Double(rxBytesDelta) / secondsDelta)
                            }
                        }
                    } else {
                        // vpn is off or new/old traffic stats are nil
                        self.bandwidthStatusModel.uploadBandwidth = BandwidthFmt.fromTransferRate(bytesPerSecond: 0)
                        self.bandwidthStatusModel.downloadBandwidth = BandwidthFmt.fromTransferRate(bytesPerSecond: 0)
                    }
                    self.bandwidthStatusModel.trafficStats = newTrafficStats
                } catch {
                    logger.error("getTrafficStats failed with error \(error, privacy: .public)")
                }
            }
        }
    }

    var body: some View {
        self.mainContent
            .overlay(
                GeometryReader { geometryProxy in
                    Color.clear
                        .preference(key: SizePreferenceKey.self, value: geometryProxy.size)
                }
            )
            .onPreferenceChange(SizePreferenceKey.self, perform: { size in
                self.sizePassthrough.send(size)
            })
    }
}

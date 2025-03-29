import Combine
import OSLog
import SwiftUI
import UserNotifications

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "StatusMenu")
private let creatingDebuggingArchiveStr = "Creating Debugging Archive (takes a few minutes)"
private let createDebuggingArchiveStr = "Create Debugging Archive"

// https://multi.app/blog/pushing-the-limits-nsstatusitem
final class StatusItemManager: ObservableObject {
    private var hostingView: NSHostingView<StatusItem>?
    private var statusItem: NSStatusItem?
    private var debuggingMenuItem: NSMenuItem?
    private var viewLatestDebugItem: NSMenuItem?
    private var accountMenuItemSeperator: NSMenuItem?
    private var accountMenuItem: NSMenuItem?

    private var sizePassthrough = PassthroughSubject<CGSize, Never>()
    private var sizeCancellable: AnyCancellable?
    private var bandwidthStatusModel = BandwidthStatusModel()
    private var osStatusModel = OsStatusModel()

    // intentionally empty to ensure that the menu item can be hightlighted
    @objc func emptyAction() {}

    func createStatusItem() {
        let statusItem: NSStatusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        let hostingView = NSHostingView(rootView: StatusItem(sizePassthrough: sizePassthrough, bandwidthStatusModel: bandwidthStatusModel, osStatusModel: self.osStatusModel))
        hostingView.frame = NSRect(x: 0, y: 0, width: 80, height: 24)
        statusItem.button?.frame = hostingView.frame
        statusItem.button?.addSubview(hostingView)

        let menu = NSMenu()

        let toggleMenuItem = NSMenuItem(title: "Toggle VPN", action: #selector(self.emptyAction), keyEquivalent: "")
        let toggleHostingView = MenuItemView(ObscuraToggle(osStatusModel: self.osStatusModel))
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

        self.accountMenuItemSeperator = NSMenuItem.separator()
        menu.addItem(self.accountMenuItemSeperator!)

        self.accountMenuItem = NSMenuItem(title: "", action: #selector(self.emptyAction), keyEquivalent: "")
        self.accountMenuItem!.isHidden = true
        self.accountMenuItem!.target = self
        menu.addItem(self.accountMenuItem!)

        Task { @MainActor in
            while true {
                if let appState = StartupModel.shared.appState {
                    if let account = appState.status.account {
                        let secondsStamp = UInt64(Date().timeIntervalSince1970)
                        var pollAccount = false
                        if (!account.isActive() || account.daysUntilExpiry() == 0) && secondsStamp - account.lastUpdatedSec > 60 * 5 {
                            pollAccount = true
                        } else if account.expiringSoon() && secondsStamp - account.lastUpdatedSec > 60 * 60 * 12 {
                            pollAccount = true
                        }
                        if pollAccount {
                            _ = try? await getAccountInfo(appState.manager)
                        }
                        let accountHostingView = MenuItemView(StatusItemAccount(account: account))
                        self.accountMenuItem!.view = accountHostingView
                        self.accountMenuItem!.isHidden = !account.expiringSoon()
                    } else {
                        self.accountMenuItem!.isHidden = true
                    }
                } else {
                    self.accountMenuItem!.isHidden = true
                }
                self.accountMenuItemSeperator!.isHidden = self.accountMenuItem!.isHidden
                do {
                    try await Task.sleep(seconds: 5)
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
            menu.addItem(bandwidthStatusTitleItem)
        }
        let bandwidthStatusItem = NSMenuItem(title: "", action: nil, keyEquivalent: "")
        bandwidthStatusItem.view = MenuItemView(BandwidthStatus(bandwidthStatusModel: self.bandwidthStatusModel))
        menu.addItem(bandwidthStatusItem)

        menu.addItem(NSMenuItem.separator())

        self.debuggingMenuItem = NSMenuItem(title: createDebuggingArchiveStr, action: #selector(self.createDebuggingArchiveAction), keyEquivalent: "")
        self.debuggingMenuItem!.target = self
        menu.addItem(self.debuggingMenuItem!)

        self.viewLatestDebugItem = NSMenuItem(title: "View Latest Debug Archive", action: #selector(self.viewLatestDebugArchive), keyEquivalent: "")
        self.viewLatestDebugItem!.isHidden = true
        self.viewLatestDebugItem!.target = self
        menu.addItem(self.viewLatestDebugItem!)

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

        Task { @MainActor in
            while true {
                if let debugBundleStatus = self.osStatusModel.osStatus?.debugBundleStatus {
                    if let debuggingMenuItem = self.debuggingMenuItem {
                        if debugBundleStatus.inProgress {
                            debuggingMenuItem.isEnabled = false
                            debuggingMenuItem.action = nil
                            debuggingMenuItem.title = creatingDebuggingArchiveStr
                            self.viewLatestDebugItem?.isHidden = true
                        } else if !debuggingMenuItem.isEnabled {
                            debuggingMenuItem.isEnabled = true
                            debuggingMenuItem.action = #selector(self.createDebuggingArchiveAction)
                            debuggingMenuItem.title = createDebuggingArchiveStr
                            let viewLatestAllowed = debugBundleStatus.latestPath != nil
                            self.viewLatestDebugItem?.isHidden = !viewLatestAllowed
                            self.viewLatestDebugItem?.isEnabled = viewLatestAllowed
                        }
                    }
                }
                do {
                    try await Task.sleep(seconds: 5)
                } catch {
                    return
                }
            }
        }
    }

    @objc func showWindow() {
        fullyOpenManagerWindow()
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
        Task {
            self.debuggingMenuItem!.isEnabled = false
            self.debuggingMenuItem!.action = nil
            self.debuggingMenuItem!.title = creatingDebuggingArchiveStr
            do {
                let _ = try await createDebuggingArchive(appState: StartupModel.shared.appState)
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
                                self.statusIconIdx = (self.statusIconIdx + self.connectingImageNames.count - 1) % self.connectingImageNames.count
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
        .task {
            while true {
                do {
                    if self.startupModel.appState == nil {
                        throw "appState is nil"
                    }
                    self.osStatusModel.osStatus = try await self.startupModel.appState?.getOsStatus(knownVersion: self.osStatusModel.osStatus?.version)
                } catch {
                    logger.error("could not update osStatsus. \(error, privacy: .public)")
                    do {
                        try await Task.sleep(seconds: 1)
                    } catch {
                        logger.info("Task cancelled \(error, privacy: .public)")
                        return // Another task will be started.
                    }
                }
            }
        }
        .task {
            var trafficStats: TrafficStats?
            do {
                while true {
                    try await Task.sleep(seconds: 1)
                    if let appState = startupModel.appState {
                        if case .connected = appState.status.vpnStatus {
                            let oldTrafficStats = trafficStats
                            trafficStats = try? await appState.getTrafficStats()
                            if let oldTrafficStats = oldTrafficStats, let newTrafficStats = trafficStats, oldTrafficStats.connId == newTrafficStats.connId {
                                let (txBytesDelta, overflowedTx) = newTrafficStats.txBytes.subtractingReportingOverflow(oldTrafficStats.txBytes)
                                let (rxBytesDelta, overflowedRx) = newTrafficStats.rxBytes.subtractingReportingOverflow(oldTrafficStats.rxBytes)
                                let (msElapsed, overflowedT) = newTrafficStats.connectedMs.subtractingReportingOverflow(oldTrafficStats.connectedMs)
                                if overflowedTx || overflowedRx || overflowedT {
                                    logger.info("oldTrafficStats: tx \(oldTrafficStats.txBytes, privacy: .public), rx \(oldTrafficStats.rxBytes, privacy: .public), timestamp \(oldTrafficStats.connectedMs, privacy: .public)")
                                    logger.info("newTrafficStats: tx \(newTrafficStats.txBytes, privacy: .public), rx \(newTrafficStats.rxBytes, privacy: .public), timestamp \(newTrafficStats.connectedMs, privacy: .public)")
                                    #if DEBUG
                                        fatalError("unexpected overflowed in bandwidth substractions. tx overflowed? \(overflowedTx), rx overflowed? \(overflowedRx), timestamp overflowed?  \(overflowedT)")
                                    #else
                                        logger.error("unexpected overflowed in bandwidth substractions. tx overflowed? \(overflowedTx, privacy: .public), rx overflowed? \(overflowedRx, privacy: .public), timestamp overflowed?  \(overflowedT, privacy: .public)")
                                    #endif
                                } else {
                                    let secondsDelta = Double(msElapsed) / 1000
                                    if secondsDelta > 0 {
                                        self.bandwidthStatusModel.uploadBandwidth = BandwidthFmt.fromTransferRate(bytesPerSecond: Double(txBytesDelta) / secondsDelta)
                                        self.bandwidthStatusModel.downloadBandwidth = BandwidthFmt.fromTransferRate(bytesPerSecond: Double(rxBytesDelta) / secondsDelta)
                                        continue
                                    }
                                }
                            }
                        }
                    }
                    self.bandwidthStatusModel.uploadBandwidth = BandwidthFmt.fromTransferRate(bytesPerSecond: 0)
                    self.bandwidthStatusModel.downloadBandwidth = BandwidthFmt.fromTransferRate(bytesPerSecond: 0)
                }
            } catch {
                logger.info("traffic task exception or cancelled \(error, privacy: .public)")
                return // Another task will be started.
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

class OsStatusModel: ObservableObject {
    @Published var osStatus: OsStatus? = nil
}

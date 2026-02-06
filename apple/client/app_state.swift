import Foundation
#if os(iOS)
    import MessageUI
    import StoreKit
#endif
import NetworkExtension
import OSLog
import SwiftUI
import UserNotifications

class AppState: ObservableObject {
    private static let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "AppState")
    var manager: NETunnelProviderManager
    private let configQueue: DispatchQueue = .init(label: "config queue")
    let osStatus: WatchableValue<OsStatus>
    @Published var status: NeStatus
    @Published var needsIsEnabledFix: Bool = false
    @Published var showOfferCodeRedemption: Bool = false

    #if !os(macOS)
        private var didBecomeActiveObserver: NSObjectProtocol?
    #endif

    #if os(macOS)
        let updater: SparkleUpdater
    #else
        let mailDelegate = MailDelegate()
        @Published var storeKitModel: StoreKitModel = .init()
        private var storeKitListener: StoreKitListener?
    #endif
    @Published var webviewsController: WebviewsController

    init(
        _ manager: NETunnelProviderManager,
        initialStatus: NeStatus
    ) {
        self.manager = manager
        self.status = initialStatus
        self.osStatus = OsStatus.watchable(manager: manager)
        #if os(macOS)
            self.updater = SparkleUpdater(osStatus: self.osStatus)
        #endif

        self.webviewsController = WebviewsController()
        self.webviewsController.initializeWebviews(appState: self)

        #if !os(macOS)
            self.didBecomeActiveObserver = NotificationCenter.default.addObserver(forName: UIApplication.didBecomeActiveNotification, object: nil, queue: .main) { [weak self] _ in
                self?.updateNeedIsEnabledFix()
            }
            self.storeKitListener = StoreKitListener(appState: self)
        #endif

        if initialStatus.autoConnect {
            Task {
                Self.logger.info("Auto-connect is enabled, waiting for internet availability before connecting")
                while true {
                    _ = await self.osStatus.waitUntil { $0.internetAvailable }
                    // Wait a little to increase the chance that the OS NE session manager realizes internet is available, otherwise the NE will fail to start connecting and restart, which can cost much more time.
                    try! await Task.sleep(seconds: 0.2)
                    if self.manager.protocolConfiguration?.includeAllNetworks == .some(true) {
                        // Wait even longer if includeAllNetworks is enabled. Otherwise the NE state tends to traverse connected->disconnected->connecting quickly without calling any of the appropriate callbacks and then gets stuck until stopped manually. This is very common on macos 14 and rare on macos 15.
                        try! await Task.sleep(seconds: 2)
                    }

                    if self.osStatus.get().internetAvailable == false {
                        Self.logger.info("Internet became unavailability before auto-connect was triggered. Retrying.")
                        continue
                    }
                    if !self.status.autoConnect {
                        Self.logger.info("Auto-connect was disabled while waiting for internet availability, not connecting")
                        return
                    }
                    if self.osStatus.get().tunnelActivated() {
                        Self.logger.info("Tunnel already activated abandoning auto-connect")
                        return
                    }

                    Self.logger.info("Auto-connecting")

                    do {
                        try await self.enableTunnel(TunnelArgs(exit: self.status.lastExit))
                    } catch {
                        Self.logger.error("Could not trigger auto connect \(error, privacy: .public)")
                        let content = UNMutableNotificationContent()
                        content.title = "Automatic connect failed"
                        content.body = "Could not connect automatically at launch."
                        content.interruptionLevel = .active
                        content.sound = UNNotificationSound.defaultCritical
                        displayNotification(.autoConnectFailed, content)
                        return
                    }

                    if await self.waitForTunnelActivation(Duration.seconds(1)) {
                        Self.logger.info("Successfully triggered auto-connect")
                        return
                    }
                    Self.logger.info("Auto-connect timed out, trying again")
                }
            }
        }

        Task { @MainActor in
            var version: UUID = initialStatus.version
            while true {
                if let status = try? await getNeStatus(self.manager, knownVersion: version) {
                    Self.logger.info("Status updated: \(debugFormat(status), privacy: .public)")
                    version = status.version
                    self.status = status
                    switch status.vpnStatus {
                    case .connecting(_, connectError: let err, _):
                        if err == "accountExpired" {
                            Self.logger.info("found connecting error accountExpired")
                            // TODO: iOS app should respond to this error OBS-1542
                            #if os(macOS)
                                // can't use openURL due to a runtime warning stating that it was called outside of a view
                                NSApp.delegate?.application?(NSApp, open: [URLs.AppAccountPage])
                            #endif
                        }
                    default:
                        break
                    }
                } else {
                    // TODO: Mark status as "unknown".
                    // https://linear.app/soveng/issue/OBS-358/status-icon-should-display-unknown-when-status-cant-be-read
                }
            }
        }

        Task {
            /* Hacky loop to keep the network extension alive.

                 After 60s of inactivty the network extension is decomissioned which has a number of downsides:

                 1. It leaks a `utunN` device (macOS bug).
                 2. It kills all active RPC calls (annoying).

                 In order to resolve this we simply ping the network extension in a loop.
             */
            while true {
                do {
                    try await self.ping()
                    try! await Task.sleep(seconds: 30)
                } catch {
                    Self.logger.error("Ping failed \(error.localizedDescription, privacy: .public)")
                    try! await Task.sleep(seconds: 5)
                }
            }
        }
    }

    func updateNeedIsEnabledFix() {
        Self.logger.info("updating need for isEnabled fix")
        Task { @MainActor in
            do {
                try await self.manager.loadFromPreferences()
                if self.manager.isEnabled {
                    Self.logger.info("manager is enabled, isEnabled fix not needed")
                    return
                }
            } catch {
                Self.logger.error("error loading NE preferences: \(error), assuming isEnabled fix is not needed")
                return
            }
            Self.logger.info("manager is disabled")
            do {
                try await self.ping()
                Self.logger.info("ping succeeded, isEnabled fix not needed")
                return
            } catch {
                Self.logger.error("ping failed: \(error)")
            }
            Self.logger.error("manager is disabled and ping failed, isEnabled fix needed")
            self.needsIsEnabledFix = true
        }
    }

    func runIsEnabledFix() {
        Task { @MainActor in
            Self.logger.info("running isEnabledFix")
            do {
                self.manager.isEnabled = true
                try await self.manager.saveToPreferences()
                self.needsIsEnabledFix = false
            } catch {
                Self.logger.error("error loading NE preferences: \(error)")
            }
        }
    }

    func setIncludeAllNetworks(enable: Bool) async throws {
        guard let proto = self.manager.protocolConfiguration else {
            throw "NEVPNManager.protocolConfiguration is nil"
        }

        Self.logger.info("setIncludeAllNetworks \(proto.includeAllNetworks, privacy: .public) → \(enable, privacy: .public)")

        if proto.includeAllNetworks == enable { return }

        proto.includeAllNetworks = enable
        do {
            try await self.manager.saveToPreferences()
            return
        } catch {
            Self.logger.error("Failed to save NEVPNManager: \(error.localizedDescription)")
        }

        do {
            try await self.manager.loadFromPreferences()
            return
        } catch {
            Self.logger.error("Failed to reload NEVPNManager: \(error.localizedDescription)")
        }

        proto.includeAllNetworks = false
        Self.logger.warning("Marking local includeAllNetworks to false as a safe default.")

        throw "Unable to save VPN configuration."
    }

    func enableTunnel(_ tunnelArgs: TunnelArgs?) async throws(String) {
        let useOnDemand = self.status.featureFlags.killSwitch ?? false
        // TODO: move this into startup flow or post feature enablement flow (https://linear.app/soveng/issue/OBS-2428)
        if useOnDemand {
            _ = await requestNotificationAuthorization()
        }

        for _ in 1 ..< 3 {
            let onDemandEnabled: Bool = await { () -> Bool in
                do {
                    try await self.manager.loadFromPreferences()
                    return self.manager.isEnabled && self.manager.isOnDemandEnabled
                } catch {
                    Self.logger.error("Failed to check onDemand status of tunnel \(error, privacy: .public)")
                    return false
                }
            }()
            // Remove once onDemand is unconditional ( https://linear.app/soveng/issue/OBS-2428 )
            let tunnelEnabled: Bool = self.manager.connection.status != .disconnected

            // Iff tunnel is already enabled update tunnel args without startVPNTunnel. Doing this unconditionally without returning would be correct as well, but NE round-trips can be a bit slow.
            if tunnelEnabled || onDemandEnabled {
                do {
                    Self.logger.log("Tunnel already active, set tunnel args")
                    let _: Empty = try await runNeCommand(self.manager, .setTunnelArgs(args: tunnelArgs, active: .none))
                    Self.logger.log("Successfully set tunnel args")
                    return
                } catch {
                    Self.logger.error("Setting tunnel args failed: \(error, privacy: .public)")
                }
            }

            // Call startVPNTunnel unconditionally, because onDemand will not not start the tunnel until there is traffic, which can be confusing.
            Self.logger.log("Starting tunnel")
            do {
                try self.manager.connection.startVPNTunnel(options: ["tunnelArgs": NSString(string: tunnelArgs.json())])
                Self.logger.log("startVPNTunnel called without error")
            } catch {
                Self.logger.error("startVPNTunnel failed \(error, privacy: .public)")
            }

            // Enable tunnel and onDemand
            do {
                try await self.manager.loadFromPreferences()
                self.manager.isOnDemandEnabled = useOnDemand
                if !self.manager.isEnabled {
                    Self.logger.info("NETunnelProviderManager is disabled, enabling")
                    self.manager.isEnabled = true
                }
                try await self.manager.saveToPreferences()
                try await self.manager.loadFromPreferences()
                return
            } catch {
                Self.logger.error("Could not set onDemand \(error, privacy: .public)")
            }
            try! await Task.sleep(seconds: 1)
        }
        Self.logger.error("Could not enable tunnel repeatedly, giving up...")
        throw errorCodeOther
    }

    func disableTunnel() async {
        Self.logger.log("Stopping tunnel")
        self.manager.isOnDemandEnabled = false
        do {
            try await self.manager.saveToPreferences()
            try await self.manager.loadFromPreferences()
        } catch {
            Self.logger.critical("Could not save NETunnelProviderManager preferences before stopping tunnel \(error, privacy: .public)")
        }
        self.manager.connection.stopVPNTunnel()
    }

    func getOsStatus(knownVersion: UUID?) async -> OsStatus {
        return await self.osStatus.getIfOrNext { current in
            current.version != knownVersion
        }
    }

    func ping() async throws(String) {
        let _: Empty = try await runNeCommand(self.manager, .ping, attemptTimeout: Duration.seconds(5), maxAttempts: 1)
    }

    func getAccountInfo() async throws(String) -> AccountInfo {
        return try await runNeCommand(self.manager, .apiGetAccountInfo)
    }

    func getTrafficStats() async throws(String) -> TrafficStats {
        return try await runNeCommand(self.manager, .getTrafficStats)
    }

    func resetUserDefaults() {
        for k in UserDefaultKeys.allKeys {
            UserDefaults.standard.removeObject(forKey: k)
        }
    }

    func waitForTunnelActivation(_ timeout: Duration) async -> Bool {
        let result = await self.osStatus.waitUntilWithTimeout(timeout) {
            switch $0.osVpnStatus {
            case .connected, .connecting, .reasserting:
                return true
            case .disconnected, .disconnecting, .invalid:
                return false
            @unknown default:
                return false
            }
        }
        return result != nil
    }

    // Unfortunately async notification iterators are not sendable, so we often need to resubscribe to state changes.
    // This function:
    //    - subscribes to state changes
    //    - checks if the initial status is unchanged (because subscribing may race with changes)
    //    - waits for a state change notification or timeout
    //    - returns the changed state if it didn't time out
    private static func waitForStateChange(connection: NEVPNConnection, initial: NEVPNStatus, maxSeconds: Double) async -> NEVPNStatus? {
        enum Event {
            case change
            case timeout
        }
        return await withTaskGroup(of: Event.self) { taskGroup in
            taskGroup.addTask {
                let notifications = NotificationCenter.default.notifications(named: .NEVPNStatusDidChange, object: connection)
                if connection.status != initial {
                    Self.logger.debug("Status already changed.")
                    return Event.change
                }
                for await _ in notifications {
                    Self.logger.debug("Status change notification received.")
                    return Event.change
                }
                if Task.isCancelled {
                    Self.logger.debug("Status change notification cancelled")
                } else {
                    Self.logger.error("Status change notification stream stopped unexpectedly.")
                }
                return Event.timeout
            }
            taskGroup.addTask {
                if let _ = try? await Task.sleep(seconds: maxSeconds) {
                    Self.logger.debug("Status change timeout.")
                    return Event.timeout
                }
                return Event.change
            }
            let event = await taskGroup.next()!
            taskGroup.cancelAll()
            return event == .timeout ? nil : connection.status
        }
    }

    private static func fetchDisconnectErrorAsErrorCode(connection: NEVPNConnection) async -> String {
        do {
            try await connection.fetchLastDisconnectError()
            self.logger.error("Failed to fetch disconnect error")
            return "failedWithoutDisconnectError"
        } catch {
            if let connectErrorCode = (error as NSError).connectErrorCode() {
                self.logger.log("Fetched connect error code: \(connectErrorCode)")
                return connectErrorCode
            }
            if (error as NSError).domain == NEVPNConnectionErrorDomain {
                switch NEVPNConnectionError(rawValue: (error as NSError).code) {
                case .noNetworkAvailable:
                    return "noNetworkAvailable"
                default:
                    Self.logger.error("Unexpected NEVPNConnectionError after startTunnel: \(error, privacy: .public)")
                    return errorCodeOther
                }
            }
            Self.logger.error("Unexpected error after startTunnel: \(error, privacy: .public)")
            return errorCodeOther
        }
    }

    #if os(iOS)
        func associateAccount() async throws(String) -> AppleAssociateAccountOutput {
            let appTransaction: String
            do {
                appTransaction = try await AppTransaction.shared.jwsRepresentation
            } catch {
                throw errorFailedToAssociateAccount
            }
            return try await runNeCommand(self.manager, .apiAppleAssociateAccount(appTransactionJws: appTransaction))
        }

        // TODO: Test interrupted purchase
        // https://developer.apple.com/documentation/storekit/testing-an-interrupted-purchase
        func purchase(product: Product) async throws -> Bool {
            _ = try await self.associateAccount()
            let result = try await product.purchase()
            if case .success(let verification) = result {
                if case .verified(let transaction) = verification {
                    await transaction.finish()
                    return true
                }
            }
            return false
        }

        func purchaseSubscription() async throws(String) -> Bool {
            guard let subscriptionProduct = await self.storeKitModel.subscriptionProduct else {
                Self.logger.error("subscription product missing")
                return false
            }
            do {
                return try await self.purchase(product: subscriptionProduct)
            } catch {
                Self.logger.error("Failed to purchase subscription: \(error, privacy: .public)")
                throw errorPurchaseFailed
            }
        }

        private func rootViewController() -> UIViewController? {
            UIApplication.shared.connectedScenes
                .compactMap { $0 as? UIWindowScene }
                .filter { $0.activationState == .foregroundActive }
                .first?.keyWindow?.rootViewController
        }

        private func presentFromRoot(viewController: UIViewController) {
            let rvc = self.rootViewController()
            // This generates a ton of spurious warnings and errors, which is
            // apparently normal. Also, the first present will be slow when
            // connected for debugging.
            rvc?.present(viewController, animated: true, completion: nil)
        }

        func emailDebugArchive(path: String, subject: String, body: String) throws(String) {
            if !MFMailComposeViewController.canSendMail() {
                Self.logger.info("Mail services are not available")
                return
            }
            let cvc = MFMailComposeViewController()
            cvc.mailComposeDelegate = self.mailDelegate
            cvc.setToRecipients(["support@obscura.net"])
            cvc.setSubject(subject)
            cvc.setMessageBody(body, isHTML: false)
            let url = URL(fileURLWithPath: path)
            let data: Data
            do {
                data = try Data(contentsOf: url)
            } catch {
                throw "Failed to read debugging archive: \(error)"
            }
            cvc.addAttachmentData(data, mimeType: "application/zip", fileName: url.lastPathComponent)
            self.presentFromRoot(viewController: cvc)
        }

        func shareFile(path: String) {
            let url = URL(fileURLWithPath: path)
            let avc = UIActivityViewController(activityItems: [url], applicationActivities: nil)
            self.presentFromRoot(viewController: avc)
        }
    #endif
}

struct TrafficStats: Codable {
    let connectedMs: UInt64
    let connId: UUID
    let txBytes: UInt64
    let rxBytes: UInt64
    let latestLatencyMs: UInt16
}

import Foundation
#if os(iOS)
    import MessageUI
#endif
import Network
import NetworkExtension
import OSLog

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "OsStatus")

class OsStatus: Encodable {
    var version: UUID = .init()
    var internetAvailable: Bool = false
    var osVpnStatus: NEVPNStatus
    let srcVersion = sourceVersion()
    var strictLeakPrevention: Bool
    var updaterStatus = UpdaterStatus()
    var debugBundleStatus = DebugBundleStatus()
    #if os(macOS)
        let canSendMail: Bool = false
    #else
        let canSendMail: Bool = MFMailComposeViewController.canSendMail()
    #endif

    struct LoginItemStatus: Codable {
        var registered: Bool
        var error: String?
    }

    var loginItemStatus: LoginItemStatus?

    init(strictLeakPrevention: Bool, osVpnStatus: NEVPNStatus) {
        self.osVpnStatus = osVpnStatus
        self.strictLeakPrevention = strictLeakPrevention
    }

    static func watchable(manager: NEVPNManager) -> WatchableValue<OsStatus> {
        var lastIncludeAllNetworks = switch manager.protocolConfiguration {
        case let .some(proto): proto.includeAllNetworks
        case nil: false // Report safe default.
        }
        let w = WatchableValue(OsStatus(
            strictLeakPrevention: lastIncludeAllNetworks,
            osVpnStatus: manager.connection.status
        ))

        #if os(macOS)
            let loginItemRegistered = isRegisteredAsLoginItem()
            w.update { value in
                value.loginItemStatus = LoginItemStatus(registered: loginItemRegistered, error: nil)
            }
        #endif
        Task {
            for await path in NWPathMonitor().stream() {
                logger.info("NWPathMonitor event: \(path.debugDescription, privacy: .public)")
                _ = w.update { value in
                    value.internetAvailable = path.status == .satisfied
                    value.version = UUID()
                }
            }
        }

        let vpnConfigNotifications = NotificationCenter.default.notifications(named: .NEVPNConfigurationChange, object: manager)
        Task {
            for await _ in vpnConfigNotifications {
                let includeAllNetworks: Bool
                if let proto = manager.protocolConfiguration {
                    includeAllNetworks = proto.includeAllNetworks
                } else {
                    logger.warning("NEVPNManager.protocolConfiguration is nil")
                    includeAllNetworks = false // Safe default
                }

                logger.info("NEVPNConfigurationChangeNotification includeAllNetworks \(includeAllNetworks, privacy: .public)")

                if includeAllNetworks == lastIncludeAllNetworks {
                    continue
                }

                lastIncludeAllNetworks = includeAllNetworks
                _ = w.update { value in
                    value.strictLeakPrevention = includeAllNetworks
                    value.version = UUID()
                }
            }
        }

        let vpnStatusNotifications = NotificationCenter.default.notifications(named: .NEVPNStatusDidChange, object: manager.connection)
        Task {
            for await _ in vpnStatusNotifications {
                let osVpnStatus = manager.connection.status
                logger.info("NEVPNStatus event: \(osVpnStatus, privacy: .public)")
                _ = w.update { value in
                    value.osVpnStatus = osVpnStatus
                    value.version = UUID()
                }
            }
        }

        return w
    }

    func tunnelActivated() -> Bool {
        switch self.osVpnStatus {
        case .connected, .connecting, .reasserting:
            return true
        case .disconnected, .disconnecting, .invalid:
            return false
        @unknown default:
            return false
        }
    }
}

// Remove this once min OS versions become macOS 14 and iOS 17
extension NWPathMonitor {
    func stream() -> AsyncStream<Network.NWPath> {
        AsyncStream { continuation in
            pathUpdateHandler = { continuation.yield($0) }
            start(queue: DispatchQueue(label: "NWPathMonitor queue"))
        }
    }
}

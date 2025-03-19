import Foundation
import Network
import NetworkExtension
import OSLog

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "OsStatus")

class OsStatus: Encodable {
    var version: UUID = .init()
    var internetAvailable: Bool = false
    var osVpnStatus: NEVPNStatus
    let srcVersion = sourceVersion()
    var updaterStatus = UpdaterStatus()
    var debugBundleStatus = DebugBundleStatus()

    init(osVpnStatus: NEVPNStatus) {
        self.osVpnStatus = osVpnStatus
    }

    static func watchable(connection: NEVPNConnection) -> WatchableValue<OsStatus> {
        let notifications = NotificationCenter.default.notifications(named: .NEVPNStatusDidChange, object: connection)
        let w = WatchableValue(OsStatus(osVpnStatus: connection.status))
        Task {
            for await path in NWPathMonitor().stream() {
                logger.info("NWPathMonitor event: \(path.debugDescription, privacy: .public)")
                _ = w.update { value in
                    value.internetAvailable = path.status == .satisfied
                    value.version = UUID()
                }
            }
        }
        Task {
            for await _ in notifications {
                let osVpnStatus = connection.status
                logger.info("NEVPNStatus event: \(osVpnStatus, privacy: .public)")
                _ = w.update { value in
                    value.osVpnStatus = osVpnStatus
                    value.version = UUID()
                }
            }
        }
        return w
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

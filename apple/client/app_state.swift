import Foundation
import NetworkExtension
import OSLog
import SwiftUI

class AppState: ObservableObject {
    private static let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "AppState")
    public var manager: NETunnelProviderManager
    private let configQueue: DispatchQueue = .init(label: "config queue")
    public let osStatus: WatchableValue<OsStatus>
    @Published var status: NeStatus

    init(
        _ manager: NETunnelProviderManager,
        initialStatus: NeStatus
    ) {
        self.manager = manager
        self.status = initialStatus
        self.osStatus = OsStatus.watchable(connection: manager.connection)

        Task { @MainActor in
            var version: UUID = initialStatus.version
            while true {
                if let status = try? await self.getStatus(knownVersion: version) {
                    Self.logger.info("Status updated: \(debugFormat(status), privacy: .public)")
                    version = status.version
                    self.status = status
                    switch status.vpnStatus {
                    case .reconnecting(exit: _, err: let err):
                        if err == "accountExpired" {
                            Self.logger.info("found reconnecting error accountExpired")
                            // can't use openURL due to a runtime warning stating that it was called outside of a view
                            NSApp.delegate?.application?(NSApp, open: [URLs.AppAccountPage])
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
                try! await Task.sleep(seconds: 30)
                do {
                    try await self.ping()
                } catch {
                    Self.logger.error("Ping failed \(error.localizedDescription, privacy: .public)")
                }
            }
        }
    }

    func accountPollSleep(daysTillExpiry: Int64?, subscriptionExpiry: Int64?) async {
        if daysTillExpiry == nil, let subscriptionExpiry = subscriptionExpiry {
            let daysTillRenewal = (subscriptionExpiry - Int64(Date().timeIntervalSince1970)) / 86400
            if daysTillRenewal > 10 {
                try! await Task.sleep(seconds: Double(min(daysTillRenewal - 10, 90) * 3600))
            } else {
                try! await Task.sleep(seconds: 24 * 3600)
            }
        } else if let daysTillExpiry = daysTillExpiry {
            if daysTillExpiry > 10 {
                try! await Task.sleep(seconds: Double(min(daysTillExpiry - 10, 90) * 3600))
            } else {
                try! await Task.sleep(seconds: 12 * 3600)
            }
        } else {
            // dead
            Self.logger.error("running account polling expected dead code")
            try! await Task.sleep(seconds: 24 * 3600)
        }
    }

    func enableTunnel(_ tunnelArgs: TunnelArgs) async throws {
        try await self.enableTunnel(jsonTunnelArgs: tunnelArgs.json())
    }

    func enableTunnel(jsonTunnelArgs: String) async throws {
        let connection = self.manager.connection
        let status = connection.status
        if status != .disconnected {
            Self.logger.error("Not starting tunnel, because it isn't disconnected: \(status, privacy: .public)")
            throw "tunnelNotDisconnected"
        }
        Self.logger.log("Starting tunnel")
        do {
            try self.manager.connection.startVPNTunnel(options: ["tunnelArgs": NSString(string: jsonTunnelArgs)])
            Self.logger.log("startVPNTunnel called without error")
        } catch {
            Self.logger.error("Error during 'startVPNTunnel': \(error.localizedDescription, privacy: .public)")
            throw errorCodeOther
        }
        // We are observed the disconnected status right before calling startVPNTunnel and expect to observe the connecting state
        // very soon after. However, startVPNTunnel may have been called from different sources (system settings, status menu, UI toggle)
        // concurrently, which is inherently racy.
        // To make sure we don't get stuck waiting for a status change that may never happen (possibly due to racy startVPNTunnel invocations)
        // we set a short timeout.
        guard let status = await Self.waitForStateChange(connection: connection, initial: status, maxSeconds: 3) else {
            Self.logger.error("Timeout waiting for first status change after 'startVPNTunnel'")
            throw errorCodeOther
        }
        switch status {
        case .connecting:
            Self.logger.log("Observed 'connecting' status after 'startVPNTunnel'")
        case .connected:
            // We missed the connecting status, or multiple attempts to call startTunnel are racing.
            // Either way, we are in the state we want to be in, so return.
            Self.logger.error("Observed 'connected' status early after 'startVPNTunnel'")
            return
        case .disconnecting, .disconnected:
            Self.logger.error("Observed \(status) status early after 'startVPNTunnel'")
            throw await Self.fetchDisconnectErrorAsErrorCode(connection: connection)
        default:
            Self.logger.error("Unexpected first status change after 'startVPNTunnel': \(status, privacy: .public)")
            throw errorCodeOther
        }

        // Unfortunately we need to subscribe again, which is racy. But we check that we are in the connecting state before we wait.
        // No matter how we ended up there, we should receive another change notification soon.
        guard let status = await Self.waitForStateChange(connection: connection, initial: status, maxSeconds: 60) else {
            Self.logger.error("Timeout waiting for status change after observing 'connecting'")
            throw errorCodeOther
        }
        switch status {
        case .connected:
            Self.logger.log("Observed 'connected' status after 'connecting'")
            return
        case .disconnecting, .disconnected:
            Self.logger.error("Observed \(status) status after 'connecting'")
            throw await Self.fetchDisconnectErrorAsErrorCode(connection: connection)
        default:
            Self.logger.log("unexpected status change after observing 'connecting': \(status, privacy: .public)")
            throw errorCodeOther
        }
    }

    func disableTunnel() {
        Self.logger.log("Stopping tunnel")
        self.manager.connection.stopVPNTunnel()
    }

    func getStatus(knownVersion: UUID?) async throws -> NeStatus {
        return try await getNeStatus(self.manager, knownVersion: knownVersion)
    }

    func getOsStatus(knownVersion: UUID?) async throws -> OsStatus {
        return await self.osStatus.getIfOrNext { current in
            current.version != knownVersion
        }
    }

    func ping() async throws {
        let cmd = ["ping": [String: String]()]
        _ = try await encodeAndRunNeJsonCommand(self.manager, cmd)
    }

    func getTrafficStats() async throws -> TrafficStats {
        let cmd = ["getTrafficStats": [String: String]()]
        let json = try await encodeAndRunNeJsonCommand(self.manager, cmd)
        return try TrafficStats(json: json)
    }

    func resetUserDefaults() {
        for k in UserDefaultKeys.allKeys {
            UserDefaults.standard.removeObject(forKey: k)
        }
    }

    // Unfortunately async notification iterators are not sendable, so we often need to resubscribe to state changes.
    // This function:
    //    - subscribes to state changes
    //    - checks if the initial status is unchanged (because subscribing may races with changes)
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
            return errorCodeOther
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
}

struct TrafficStats: Codable {
    let timestampMs: UInt64
    let connId: UUID
    let txBytes: UInt64
    let rxBytes: UInt64
}

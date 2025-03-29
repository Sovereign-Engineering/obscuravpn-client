import Foundation
import Network
import NetworkExtension
import OSLog
import SystemExtensions

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "PacketTunnelProvider")

enum TunnelProviderInitStatus: String {
    case checking
    case blockingBeforePermissionPopup
    case waitingForUserApproval
    case configuring
    case testingCommunication
    case permissionDenied
    case unexpectedError
}

enum TunnelProviderInitEvent {
    case status(TunnelProviderInitStatus)
    case done(NETunnelProviderManager, NeStatus)
}

class TunnelProviderInit {
    var continuation: AsyncStream<TunnelProviderInitEvent>.Continuation?

    func start() -> AsyncStream<TunnelProviderInitEvent> {
        return AsyncStream<TunnelProviderInitEvent> { continuation in
            self.continuation = continuation
            self.update(.checking)
            Task {
                guard let managers = await Self.loadManagers() else {
                    self.update(.unexpectedError)
                    return
                }
                if managers.count > 1 {
                    for manager in managers[1...] {
                        do {
                            logger.log("Removing extra tunnel provider: \(manager.localizedDescription ?? "nil", privacy: .public)")
                            try await manager.removeFromPreferences()
                        } catch {
                            logger.error("error removing extra tunnel provider: \(error)")
                            self.update(.unexpectedError)
                            return
                        }
                    }
                }
                if managers.isEmpty {
                    // There are no managers, we will get a permission prompt when we add one. Wait for a call to `cont()`, so we can prepare the user for the popup.
                    self.update(.blockingBeforePermissionPopup)
                } else {
                    // There already is a manager we can use, no permission promp will be shown, continue automatically.
                    self.continueAfterPermissionPriming()
                }
            }
        }
    }

    func continueAfterPermissionPriming() {
        Task {
            guard let managers = await Self.loadManagers() else {
                self.update(.unexpectedError)
                return
            }

            if managers.isEmpty {
                self.update(.waitingForUserApproval)
            } else {
                self.update(.configuring)
            }

            let manager = switch managers.first {
            case .some(let manager): manager
            case .none: NETunnelProviderManager()
            }

            let proto = NETunnelProviderProtocol()
            proto.providerBundleIdentifier = extensionBundleID()
            proto.serverAddress = "obscura.net"
            manager.protocolConfiguration = proto
            manager.isEnabled = true

            do {
                try await manager.saveToPreferences()
            } catch {
                logger.error("error saving tunnel provider to preferences early: \(error)")
                if (error as NSError).domain == NEVPNErrorDomain {
                    switch NEVPNError.Code(rawValue: (error as NSError).code) {
                    case .configurationReadWriteFailed:
                        self.update(.permissionDenied)
                    default:
                        self.update(.unexpectedError)
                    }
                }
                return
            }

            do {
                try await manager.loadFromPreferences()
            } catch {
                logger.error("error loading tunnel provider from preferences: \(error)")
                self.update(.unexpectedError)
                return
            }

            self.update(.testingCommunication)

            var pingFailures = 0
            while true {
                do {
                    let status = try await getNeStatus(
                        manager,
                        knownVersion: nil,
                        attemptTimeout: .seconds(10),
                        maxAttempts: 3
                    )
                    self.done(manager, status)
                    return
                } catch {
                    logger.error("Ping error: \(error, privacy: .public)")
                }

                pingFailures += 1
                if pingFailures > 2 {
                    logger.error("Failed to reach tunnel provider.")
                    self.update(.unexpectedError)
                    return
                }

                do {
                    logger.log("Forcing network extension init")
                    try manager.connection.startVPNTunnel(options: ["dontStartTunnel": NSString(string: "")])
                } catch {
                    logger.error("Forced network extension init failed: \(error)")
                }
            }
        }
    }

    private func update(_ status: TunnelProviderInitStatus) {
        logger.log("TunnelProviderInit status: \(status.rawValue, privacy: .public)")
        if let cont = self.continuation {
            cont.yield(.status(status))
        }
    }

    private func done(_ manager: NETunnelProviderManager, _ status: NeStatus) {
        if let cont = self.continuation {
            cont.yield(.done(manager, status))
            cont.finish()
        }
    }

    private static func loadManagers() async -> [NETunnelProviderManager]? {
        do {
            let managers: [NETunnelProviderManager] = try await NETunnelProviderManager.loadAllFromPreferences()
            return managers
        } catch {
            logger.error("loading all tunnel providers from preferences failed with error: \(error)")
            return .none
        }
    }
}

func getNeStatus(
    _ manager: NETunnelProviderManager,
    knownVersion: UUID?,
    attemptTimeout: Duration? = nil,
    maxAttempts: UInt = 10
) async throws -> NeStatus {
    try await runNeCommand(manager, NeManagerCmd.getStatus(knownVersion: knownVersion), attemptTimeout: attemptTimeout, maxAttempts: maxAttempts)
}

func getAccountInfo(
    _ manager: NETunnelProviderManager,
    attemptTimeout: Duration? = nil,
    maxAttempts: UInt = 10
) async throws -> AccountInfo {
    return try await runNeCommand(manager, NeManagerCmd.apiGetAccountInfo, attemptTimeout: attemptTimeout, maxAttempts: maxAttempts)
}

func runNeCommand<T: Codable>(
    _ manager: NETunnelProviderManager,
    _ cmd: NeManagerCmd,
    attemptTimeout: Duration? = .seconds(10),
    maxAttempts: UInt = 10
) async throws(String) -> T {
    return try T(json: await runNeJsonCommand(manager, cmd.json(), attemptTimeout: attemptTimeout, maxAttempts: maxAttempts))
}

func runNeJsonCommand(
    _ manager: NETunnelProviderManager,
    _ jsonCmd: String,
    attemptTimeout: Duration?,
    maxAttempts: UInt = 10
) async throws(String) -> String {
    var result: NeManagerCmdResult
    do {
        let resultJson = try await manager.sendAppMessage(
            jsonCmd.data(using: .utf8)!,
            maxAttempts: maxAttempts, attemptTimeout: attemptTimeout
        )
        result = try NeManagerCmdResult(json: resultJson)
    } catch {
        logger.error("could not run ne command: \(error, privacy: .public)")
        result = .error(errorCodeOther)
    }
    switch result {
    case .ok_json(let ok):
        logger.debug("ne command success")
        return ok
    case .error(let error):
        logger.debug("ne command error: \(error, privacy: .public)")
        throw error
    }
}

extension NETunnelProviderManager {
    // TODO: Merge into runNeCommand without retry logic once we are confident that the UI handles errors and necessary retries for all commands nicely.
    func sendAppMessage(
        _ msg: Data,
        maxAttempts: UInt,
        attemptTimeout: Duration?
    ) async throws -> Data {
        guard let connection = self.connection as? NETunnelProviderSession else {
            throw "NETunnelProviderManager.connection is not a NETunnelProviderSession, got \(debugFormat(self.connection))"
        }

        for attempt in 0 ..< maxAttempts {
            let clock = SuspendingClock.now
            let response = try? await withTimeout(attemptTimeout) {
                await withCheckedContinuation { continuation in
                    do {
                        logger.debug("calling sendProviderMessage")
                        try connection.sendProviderMessage(msg) { response in
                            logger.debug("sendProviderMessage returned")
                            continuation.resume(returning: response)
                        }
                    } catch {
                        logger.warning("sendProviderMessage failed: \(error, privacy: .public)")
                        continuation.resume(returning: .none)
                    }
                }
            }
            if let response = response {
                return response
            }
            let latency = SuspendingClock.now - clock
            logger.log("sendProviderMessage message failed or lost after \(latency, privacy: .public), attempt: \(attempt, privacy: .public)")
            try await Task.sleep(seconds: 1.0)
        }
        throw "sendProviderMessage message lost repeatedly"
    }
}

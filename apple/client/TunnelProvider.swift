import Foundation
import Network
import NetworkExtension
import OSLog

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "PacketTunnelProvider")

enum TunnelProviderInitStatus {
    case checking
    case blockingBeforePermissionPopup
    case waitingForUserPermissionApproval
    case waitingForUserStopOtherTunnelApproval(manager: NETunnelProviderManager)
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
                    // There are no managers, we will get a permission prompt when we add one. Wait for a call to `continueAfterPermissionPriming()`, so we can prepare the user for the popup.
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

            var askedForUserApproval = false
            if managers.isEmpty {
                self.update(.waitingForUserPermissionApproval)
                askedForUserApproval = true
            } else {
                self.update(.configuring)
            }

            let manager = switch managers.first {
            case .some(let manager): manager
            case .none: NETunnelProviderManager()
            }

            manager.onDemandRules = [NEOnDemandRuleConnect()]

            let proto = NETunnelProviderProtocol()
            proto.providerBundleIdentifier = networkExtensionBundleID()
            proto.serverAddress = "obscura.net"
            proto.includeAllNetworks = manager.protocolConfiguration?.includeAllNetworks ?? false
            manager.protocolConfiguration = proto

            do {
                if askedForUserApproval {
                    manager.isEnabled = true
                }
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

            if manager.isEnabled {
                self.continueAfterStopOtherTunnelPriming(manager)
            } else {
                logger.info("tunnel provider is not enabled, asking for permission to enable (which kills other tunnels)")
                self.update(.waitingForUserStopOtherTunnelApproval(manager: manager))
            }
        }
    }

    func continueAfterStopOtherTunnelPriming(_ manager: NETunnelProviderManager) {
        Task {
            do {
                if !manager.isEnabled {
                    logger.info("enabling tunnel provider")
                    manager.isEnabled = true
                    try await manager.saveToPreferences()
                }
            } catch {
                logger.error("error saving tunnel provider to preferences after late enablement: \(error)")
                self.update(.unexpectedError)
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
        logger.log("TunnelProviderInit status: \(debugFormat(status), privacy: .public)")
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

func neLogin(_ manager: NETunnelProviderManager,
             accountId: String,
             attemptTimeout: Duration? = nil,
             maxAttempts: UInt = 10) async throws
{
    _ = try await runNeJsonCommand(manager, NeManagerCmd.login(accountId: accountId, validate: false).json(), name: "login", attemptTimeout: attemptTimeout, maxAttempts: maxAttempts)
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

func getExitList(_ manager: NETunnelProviderManager,
                 knownVersion: String?,
                 attemptTimeout: Duration? = nil,
                 maxAttempts: UInt = 10) async throws -> CachedValue<ExitList>
{
    return try await runNeCommand(manager, NeManagerCmd.getExitList(knownVersion: knownVersion), attemptTimeout: attemptTimeout, maxAttempts: maxAttempts)
}

func refreshExitList(_ manager: NETunnelProviderManager,
                     freshness: TimeInterval,
                     attemptTimeout: Duration? = nil,
                     maxAttempts: UInt = 10) async throws -> CachedValue<ExitList>
{
    return try await runNeCommand(manager, NeManagerCmd.refreshExitList(freshness: freshness), attemptTimeout: attemptTimeout, maxAttempts: maxAttempts)
}

struct CachedValue<T: Codable>: Codable {
    var version: String
    var last_updated: TimeInterval
    var value: T
}

struct ExitList: Codable {
    var exits: [OneExit]
}

struct CityExit: Hashable {
    var city_code: String
    var country_code: String
}

struct OneExit: Codable {
    var id: String
    var city_code: String
    var country_code: String
    var city_name: String
    var provider_id: String
    var provider_url: String
    var provider_name: String
    var provider_homepage_url: String
    var datacenter_id: UInt32
    var tier: UInt8
}

func getCityNames(_ manager: NETunnelProviderManager, knownVersion: String?) async throws -> (cityNames: [CityExit: String], version: String) {
    let cachedValue = try await getExitList(manager, knownVersion: knownVersion)
    var newCityNames: [CityExit: String] = [:]
    for exit in cachedValue.value.exits {
        newCityNames[CityExit(city_code: exit.city_code, country_code: exit.country_code)] = exit.city_name
    }
    return (cityNames: newCityNames, version: cachedValue.version)
}

func runNeCommand<T: Codable>(
    _ manager: NETunnelProviderManager,
    _ cmd: NeManagerCmd,
    attemptTimeout: Duration? = .seconds(10),
    maxAttempts: UInt = 10
) async throws(String) -> T {
    return try T(json: await runNeJsonCommand(manager, cmd.json(), name: getEnumCaseName(for: cmd), attemptTimeout: attemptTimeout, maxAttempts: maxAttempts))
}

func runNeJsonCommand(
    _ manager: NETunnelProviderManager,
    _ jsonCmd: String,
    name: String?,
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
        logger.error("could not run ne command \(name, privacy: .public): \(error, privacy: .public)")
        result = .error(errorCodeOther)
    }
    switch result {
    case .ok_json(let ok):
        logger.debug("ne command \(name, privacy: .public) success")
        return ok
    case .error(let error):
        logger.debug("ne command \(name, privacy: .public) error: \(error, privacy: .public)")
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

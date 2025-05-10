import AppKit
import Foundation
import OSLog
import ServiceManagement

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "command")

enum Command: Codable {
    case startTunnel(tunnelArgs: String)
    case stopTunnel
    case debuggingArchive
    case revealItemInDir(path: String)
    case setStrictLeakPrevention(enable: Bool)
    case registerAsLoginItem
    case unregisterAsLoginItem
    case isRegisteredAsLoginItem
    case resetUserDefaults
    case getOsStatus(knownVersion: UUID?)
    case checkForUpdates
    case installUpdate
    case jsonFfiCmd(
        cmd: String,
        timeoutMs: Int?
    )
}

func handleWebViewCommand(command: Command) async throws(String) -> String {
    guard let appState = StartupModel.shared.appState else {
        logger.critical("received web view command before `appState` was initialized")
        throw errorCodeOther
    }
    switch command {
    case .startTunnel(tunnelArgs: let jsonArgs):
        let args = try TunnelArgs(json: jsonArgs)
        try await appState.enableTunnel(args)
    case .stopTunnel:
        appState.disableTunnel()
    case .debuggingArchive:
        let path: String
        do {
            path = try await createDebuggingArchive(appState: appState)
        } catch {
            logger.error("could not create debugging archive \(error, privacy: .public)")
            throw errorCodeOther
        }
        return try path.json()
    case .revealItemInDir(let path):
        NSWorkspace.shared.selectFile(path, inFileViewerRootedAtPath: "")
    case .registerAsLoginItem:
        try registerAsLoginItem()
    case .unregisterAsLoginItem:
        try unregisterAsLoginItem()
    case .isRegisteredAsLoginItem:
        return try isRegisteredAsLoginItem().json()
    case .resetUserDefaults:
        // NOTE: only shown in the Developer View
        appState.resetUserDefaults()
    case .setStrictLeakPrevention(let enable):
        do {
            try await appState.setIncludeAllNetworks(enable: enable)
        } catch {
            logger.error("Could not set includeAllNetworks \(error, privacy: .public)")
            throw errorCodeOther
        }
    case .jsonFfiCmd(cmd: let jsonCmd, let timeoutMs):
        let attemptTimeout: Duration? = switch timeoutMs {
        case .some(let ms): .milliseconds(ms)
        case .none: nil
        }
        return try await runNeJsonCommand(
            appState.manager,
            jsonCmd,
            attemptTimeout: attemptTimeout
        )
    case .getOsStatus(knownVersion: let version):
        return try await appState.getOsStatus(knownVersion: version).json()
    case .checkForUpdates:
        #if os(macOS)
            guard appState.updater.canCheckForUpdates else {
                throw errorCodeUpdaterCheck
            }
            appState.updater.checkForUpdates()
        #else
            throw errorCodeUpdaterCheck
        #endif
    case .installUpdate:
        #if os(macOS)
            guard appState.updater.canCheckForUpdates else {
                throw errorCodeUpdaterInstall
            }
            appState.updater.showUpdaterIfNeeded()
        #else
            throw errorCodeUpdaterInstall
        #endif
    }
    return "{}"
}

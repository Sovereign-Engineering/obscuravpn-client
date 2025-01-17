import Foundation
import OSLog
import ServiceManagement

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "command")

enum Command: Codable {
    case startTunnel(tunnelArgs: String)
    case stopTunnel
    case debuggingArchive
    case registerLoginItem
    case resetUserDefaults
    case getOsStatus(knownVersion: UUID?)
    case jsonFfiCmd(
        cmd: String,
        timeoutMs: Int?
    )
}

func handleWebViewCommand(command: Command) async throws -> String {
    guard let appState = StartupModel.shared.appState else {
        logger.critical("received web view command before `appState` was initialized")
        throw errorCodeOther
    }
    switch command {
    case .startTunnel(tunnelArgs: let args):
        try await appState.enableTunnel(jsonTunnelArgs: args)
    case .stopTunnel:
        appState.disableTunnel()
    case .debuggingArchive:
        try await createDebuggingArchive()
    case .registerLoginItem:
        return try registerLoginitem().json()
    case .resetUserDefaults:
        // NOTE: only shown in the Developer View
        appState.resetUserDefaults()
    case .jsonFfiCmd(cmd: let jsonCmd, let timeoutMs):
        let attemptTimeout: Duration? = switch timeoutMs {
        case .some(let ms): .milliseconds(ms)
        case .none: nil
        }
        return try await runNeCommand(
            appState.manager,
            jsonCmd,
            attemptTimeout: attemptTimeout
        )
    case .getOsStatus(knownVersion: let version):
        return try await appState.getOsStatus(knownVersion: version).json()
    }
    return "{}"
}

func registerLoginitem() -> Bool {
    do {
        try SMAppService.mainApp.register()
        return true
    } catch {
        logger.info("failed to register app at login")
    }
    return false
}

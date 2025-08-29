#if os(macOS)
    import AppKit
#endif
import Foundation
import OSLog

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "command")

enum Command: Codable {
    case startTunnel(tunnelArgs: String)
    case stopTunnel
    case setStrictLeakPrevention(enable: Bool)
    case setColorScheme(value: AppAppearance)
    case debuggingArchive
    case revealItemInDir(path: String)
    case registerAsLoginItem
    case unregisterAsLoginItem
    case resetUserDefaults
    case getOsStatus(knownVersion: UUID?)
    case checkForUpdates
    case installUpdate
    case jsonFfiCmd(
        cmd: String,
        timeoutMs: Int?
    )
}

extension CommandHandler {
    func handleWebViewCommand(command: Command) async throws(String) -> String {
        switch command {
        case .startTunnel(tunnelArgs: let jsonArgs):
            let args = try TunnelArgs(json: jsonArgs)
            try await appState.enableTunnel(args)
        case .stopTunnel:
            appState.disableTunnel()
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
        case .setColorScheme(let colorScheme):
            DispatchQueue.main.async {
                StartupModel.shared.selectedAppearance = colorScheme
            }
            // When setting color scheme to no preference (nil),
            //  only the header changes appearance immediately
            // Setting to nil a second time results in the expected visual change
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.01) {
                StartupModel.shared.selectedAppearance = colorScheme
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
        #if os(macOS)
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
                try registerAsLoginItem(appState: self.appState)
            case .unregisterAsLoginItem:
                try unregisterAsLoginItem(appState: self.appState)
            case .checkForUpdates:
                try? appState.updater.checkForUpdates()
            case .installUpdate:
                guard appState.updater.canCheckForUpdates else {
                    throw errorCodeUpdaterInstall
                }
                appState.updater.showUpdaterIfNeeded()
        #else
            case .debuggingArchive, .revealItemInDir, .registerAsLoginItem, .unregisterAsLoginItem, .checkForUpdates, .installUpdate:
                throw errorUnsupportedOnOS
        #endif
        }
        return "{}"
    }
}

import OSLog
import ServiceManagement

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "loginitem")

func unregisterAsLoginItem(appState: AppState) throws(String) {
    do {
        try SMAppService.mainApp.unregister()
        let loginItemRegistered = isRegisteredAsLoginItem()
        _ = appState.osStatus.update { value in
            value.loginItemStatus = OsStatus.LoginItemStatus(registered: loginItemRegistered, error: nil)
        }
    } catch {
        _ = appState.osStatus.update { value in
            value.loginItemStatus?.error = error.localizedDescription
        }
        logger.error("failed to unregister app at login \(error, privacy: .public)")
        throw errorCodeOther
    }
}

func registerAsLoginItem(appState: AppState?) throws(String) {
    do {
        try SMAppService.mainApp.register()
        let loginItemRegistered = isRegisteredAsLoginItem()
        if let appState = appState {
            _ = appState.osStatus.update { value in
                value.loginItemStatus = OsStatus.LoginItemStatus(registered: loginItemRegistered, error: nil)
            }
        }
    } catch {
        if let appState = appState {
            _ = appState.osStatus.update { value in
                value.loginItemStatus?.error = error.localizedDescription
            }
        }
        logger.error("failed to register app at login \(error, privacy: .public)")
        throw errorCodeOther
    }
}

func isRegisteredAsLoginItem() -> Bool {
    return SMAppService.mainApp.status == .enabled
}

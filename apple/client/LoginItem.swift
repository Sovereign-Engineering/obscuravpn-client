import OSLog
import ServiceManagement

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "loginitem")

func unregisterAsLoginItem() throws(String) {
    do {
        try SMAppService.mainApp.unregister()
    } catch {
        logger.error("failed to unregister app at login \(error, privacy: .public)")
        throw errorCodeOther
    }
}

func registerAsLoginItem() throws(String) {
    do {
        try SMAppService.mainApp.register()
    } catch {
        logger.error("failed to register app at login \(error, privacy: .public)")
        throw errorCodeOther
    }
}

func isRegisteredAsLoginItem() -> Bool {
    return SMAppService.mainApp.status == .enabled
}

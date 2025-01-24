import OSLog
import ServiceManagement

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "loginitem")

func unregisterAsLoginItem() throws {
    do {
        try SMAppService.mainApp.unregister()
    } catch {
        logger.info("failed to unregister app at login")
        throw error
    }
}

func registerAsLoginItem() throws {
    do {
        try SMAppService.mainApp.register()
    } catch {
        logger.info("failed to register app at login")
        throw error
    }
}

func isRegisteredAsLoginItem() -> Bool {
    return SMAppService.mainApp.status == .enabled
}

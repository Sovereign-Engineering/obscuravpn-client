import OSLog
import Sparkle

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "UpdaterDriver")

class UpdaterDriver: NSObject, SPUUserDriver {
    private var osStatus: WatchableValue<OsStatus>

    static func createUpdater(osStatus: WatchableValue<OsStatus>) -> SPUUpdater {
        let updater = SPUUpdater(hostBundle: Bundle.main, applicationBundle: Bundle.main, userDriver: UpdaterDriver(osStatus: osStatus), delegate: nil)
        do {
            try updater.start()
        } catch {
            logger.error("Error starting custom updater: \(error, privacy: .public)")
        }
        return updater
    }

    init(osStatus: WatchableValue<OsStatus>) {
        self.osStatus = osStatus
        super.init()
    }

    private func updateOsStatus(updaterStatus: UpdaterStatus) {
        logger.info("New osStatus.updaterStatus \(updaterStatus, privacy: .public))")
        _ = self.osStatus.update { value in
            value.updaterStatus = updaterStatus
            value.version = UUID()
        }
    }

    func showUserInitiatedUpdateCheck(cancellation: @escaping () -> Void) {
        let status = UpdaterStatus(type: .initiated, appcast: nil, error: nil, errorCode: nil)
        self.updateOsStatus(updaterStatus: status)
    }

    func showUpdateFound(with appcastItem: SUAppcastItem, state: SPUUserUpdateState) async -> SPUUserUpdateChoice {
        let appcast = AppcastSummary(
            date: appcastItem.dateString ?? "",
            description: appcastItem.itemDescription ?? "",
            version: appcastItem.displayVersionString,
            minSystemVersionOk: appcastItem.minimumOperatingSystemVersionIsOK
        )
        let status = UpdaterStatus(type: .available, appcast: appcast, error: nil, errorCode: nil)
        self.updateOsStatus(updaterStatus: status)
        // don't want to install it
        return .dismiss
    }

    func showUpdateNotFoundWithError(_ error: Error, acknowledgement: @escaping () -> Void) {
        let appcastItem = (error as NSError).userInfo[SPULatestAppcastItemFoundKey] as? SUAppcastItem
        let notFoundReason = (error as NSError).userInfo[SPUNoUpdateFoundReasonKey] as? Int32

        let appcast = appcastItem.map { item in
            AppcastSummary(
                date: item.dateString ?? "",
                description: item.itemDescription ?? "",
                version: item.displayVersionString,
                minSystemVersionOk: item.minimumOperatingSystemVersionIsOK
            )
        }

        let status = UpdaterStatus(type: .notFound, appcast: appcast, error: error.localizedDescription, errorCode: notFoundReason)
        self.updateOsStatus(updaterStatus: status)
        acknowledgement()
    }

    func showUpdaterError(_ error: Error, acknowledgement: @escaping () -> Void) {
        let status = UpdaterStatus(type: .error, appcast: nil, error: error.localizedDescription, errorCode: nil)
        self.updateOsStatus(updaterStatus: status)
        acknowledgement()
    }

    func show(_ request: SPUUpdatePermissionRequest) async -> SUUpdatePermissionResponse {
        return SUUpdatePermissionResponse(automaticUpdateChecks: false, sendSystemProfile: false)
    }

    func showUpdateReleaseNotes(with downloadData: SPUDownloadData) {}

    func showUpdateReleaseNotesFailedToDownloadWithError(_ error: Error) {}

    func showDownloadInitiated(cancellation: @escaping () -> Void) {}

    func showDownloadDidReceiveExpectedContentLength(_ expectedContentLength: UInt64) {}

    func showDownloadDidReceiveData(ofLength length: UInt64) {}

    func showDownloadDidStartExtractingUpdate() {}

    func showExtractionReceivedProgress(_ progress: Double) {}

    func showReadyToInstallAndRelaunch() async -> SPUUserUpdateChoice {
        return .install
    }

    func showInstallingUpdate(withApplicationTerminated applicationTerminated: Bool, retryTerminatingApplication: @escaping () -> Void) {}

    func showUpdateInstalledAndRelaunched(_ relaunched: Bool, acknowledgement: @escaping () -> Void) {
        acknowledgement()
    }

    func showUpdateInFocus() {}

    func dismissUpdateInstallation() {}
}

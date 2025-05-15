import Combine
import Sparkle

class SparkleUpdater {
    /**
     Sparkle updater.

     - seealso: [How to integrate the Sparkle framework into a SwiftUI app for MacOS](https://medium.com/@matteospada.m/how-to-integrate-the-sparkle-framework-into-a-swiftui-app-for-macos-98ca029f83f7)
     - seealso: [Sparkle: Basic Setup](https://sparkle-project.org/documentation/)
     - seealso: [Sparkle: Create an Updater in SwiftUI](https://sparkle-project.org/documentation/programmatic-setup/#create-an-updater-in-swiftui)
     */
    private let sparkleUpdater: SPUUpdater
    private let updaterController: SPUStandardUpdaterController

    init(osStatus: WatchableValue<OsStatus>) {
        self.updaterController = SPUStandardUpdaterController(startingUpdater: true, updaterDelegate: nil, userDriverDelegate: nil)
        self.sparkleUpdater = UpdaterDriver.createUpdater(osStatus: osStatus)
    }

    var sessionInProgress: Bool {
        return self.sparkleUpdater.sessionInProgress
    }

    var canCheckForUpdates: Bool {
        return self.sparkleUpdater.canCheckForUpdates
    }

    func checkForUpdates() {
        if self.sessionInProgress {
            return
        }
        guard self.canCheckForUpdates else {
            throw errorCodeUpdaterCheck
        }
        self.sparkleUpdater.checkForUpdates()
    }

    func showUpdaterIfNeeded() {
        self.updaterController.checkForUpdates(nil)
    }

    var canCheckForUpdatesPublisher: AnyPublisher<Bool, Never> {
        self.sparkleUpdater.publisher(for: \.canCheckForUpdates).eraseToAnyPublisher()
    }
}

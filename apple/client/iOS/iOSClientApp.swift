import OSLog
import SwiftUI

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "App")

@main
struct iOSClientApp: App {
    init() {
        logger.debug("App init")
    }

    @ObservedObject var startupModel = StartupModel.shared

    var body: some Scene {
        WindowGroup {
            if let appState = self.startupModel.appState {
                ContentView(appState: appState)
            } else {
                StartupView()
                    .onAppear {
                        if let colorSchemeValue = UserDefaults.standard.string(forKey: UserDefaultKeys.Appearance), let colorSchemeSelected = ColorScheme(rawValue: colorSchemeValue) {
                            setAppearance(colorScheme: colorSchemeSelected)
                        }
                    }
            }
        }
    }
}

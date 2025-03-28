import Network
import NetworkExtension
import OSLog
import Sparkle
import SwiftUI
import SystemExtensions
import UserNotifications

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "App")

func openWindow(id: String) {
    NSApp.windows
        .filter { $0.identifier?.rawValue == id }
        .forEach {
            $0.makeKeyAndOrderFront(nil)
            $0.orderFrontRegardless()
        }
}

// dismissWindow(id: String) requires macOS 14+
func closeWindow(id: String) {
    NSApp.windows
        .filter { $0.identifier?.rawValue == id }
        .forEach { $0.close() }
}

class AppDelegate: NSObject, NSApplicationDelegate, UNUserNotificationCenterDelegate, ObservableObject {
    func applicationWillFinishLaunching(_ notification: Notification) {
        UNUserNotificationCenter.current().delegate = self
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        // https://stackoverflow.com/a/19890943/7732434
        let event = NSAppleEventManager.shared().currentAppleEvent
        let launchedAsLoginItem = (event?.eventID == kAEOpenApplication && event?.paramDescriptor(forKeyword: keyAEPropData)?.enumCodeValue == keyAELaunchedAsLogInItem)
        logger.log("launched as login item: \(launchedAsLoginItem)")
        if launchedAsLoginItem {
            // Without this, the window does show up
            // With this, the icon never even comes on the dock
            closeWindow(id: WindowIds.RootWindowId)
            // Therefore, in the future, just check if the user wants the window to show up before closing
        }
    }

    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        logger.debug("from applicationShouldTerminateAfterLastWindowClosed")
        // Without this workaround, if the user closes the window using a keyboard shortcut that highlights an
        // App Menu item (e.g. Command-Q or Command-W) and tries to open it again there will be either:
        //   - A brief flash of highlight of the App Menu item on next start
        //   - A persistent highlight of the App Menu item (when it's a "reopen" via double-clicking on Finder or similar)
        NSRunningApplication.runningApplications(withBundleIdentifier: "com.apple.dock").first?.activate(options: [])
        OperationQueue.current?.underlyingQueue?.asyncAfter(deadline: .now() + .milliseconds(200)) {
            NSApp.setActivationPolicy(.accessory)
        }
        return false
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows: Bool) -> Bool {
        logger.debug("from applicationShouldHandleReopen. hasVisibleWindows = \(hasVisibleWindows)")
        // On macos ventura or earlier, without this workaround, if the user
        // reopens the App using Finder while the App is already running, the
        // App Menu (the left side) becomes completely frozen and unusable
        // (even the  one)
        /// more info here:
        // https://linear.app/soveng/issue/OBS-175/no-obscura-vpn-in-menu-bar-dock-or-app-switcher-when-application-is#comment-2ecf3e57
        NSApp.setActivationPolicy(.regular)
        NSRunningApplication.runningApplications(withBundleIdentifier: "com.apple.dock").first?.activate(options: [])
        OperationQueue.current?.underlyingQueue?.asyncAfter(deadline: .now() + .milliseconds(200)) {
            NSApp.activate(ignoringOtherApps: true)
            openWindow(id: WindowIds.RootWindowId)
        }
        return true
    }

    func userNotificationCenter(
        _ center: UNUserNotificationCenter,
        willPresent notification: UNNotification
    ) async -> UNNotificationPresentationOptions {
        // Always show notifications, even if we have focus.
        // Right now we use notifications as the only feedback for some actions.
        // This is probably not ideal UX but until we can improve that ensure that they appear on screen.
        return .banner
    }

    var openUrlCallback: ((_ url: URL) -> Void)?

    func application(
        _ application: NSApplication,
        open urls: [URL]
    ) {
        logger.log("AppDelegate \(#function) called with URLs: \(urls)")
        guard let openUrlCallback = self.openUrlCallback else {
            logger.warning("AppDelegate has NO registered openUrlCallback")
            return
        }

        logger.log("AppDelegate: Calling registered openUrlCallback")
        for url in urls {
            openUrlCallback(url)
        }
    }
}

func fullyOpenManagerWindow() {
    NSApp.setActivationPolicy(.regular)
    // Focus must be done before opening the window, otherwise it's possible to not have focus
    focusApp()
    openWindow(id: WindowIds.RootWindowId)
}

@main
struct ClientApp: App {
    /**
     Sparkle updater.

     - seealso: [How to integrate the Sparkle framework into a SwiftUI app for MacOS](https://medium.com/@matteospada.m/how-to-integrate-the-sparkle-framework-into-a-swiftui-app-for-macos-98ca029f83f7)
     - seealso: [Sparkle: Basic Setup](https://sparkle-project.org/documentation/)
     - seealso: [Sparkle: Create an Updater in SwiftUI](https://sparkle-project.org/documentation/programmatic-setup/#create-an-updater-in-swiftui)
     */
    private let updaterController: SPUStandardUpdaterController

    init() {
        logger.debug("App init")
        // Auto-exit if app is already running
        // Note that this is already rare, but can happen if an installed app is running before running a build from XCode
        if NSWorkspace.shared.runningApplications.filter({ $0.bundleIdentifier == Bundle.main.bundleIdentifier }).count > 1 {
            logger.info("App already running.")
            NSApp.terminate(nil)
        }
        self.updaterController = .init(startingUpdater: true, updaterDelegate: nil, userDriverDelegate: nil)
    }

    @NSApplicationDelegateAdaptor private var appDelegate: AppDelegate
    @ObservedObject var startupModel = StartupModel.shared
    @StateObject private var statusManager = StatusItemManager()

    var body: some Scene {
        Window("Obscura", id: WindowIds.RootWindowId) {
            Group {
                if let appState = self.startupModel.appState {
                    ContentView(appState: appState, updaterController: self.updaterController)
                        .frame(minWidth: 805, minHeight: 525)
                } else {
                    StartupView()
                }
            }.onAppear {
                // this must be called under a View. If put in init it will fail
                self.statusManager.createStatusItem()
            }
        }.commands {
            CommandGroup(replacing: CommandGroupPlacement.appTermination) {
                Button("Close Window") {
                    closeWindow(id: WindowIds.RootWindowId)
                }.keyboardShortcut("q")
            }

            CommandGroup(after: .appInfo) {
                CheckForUpdatesView(updater: self.updaterController.updater)
            }
        }
    }
}

func focusApp() {
    if #available(macOS 14.0, *) {
        NSApp.activate()
    } else {
        NSApp.activate(ignoringOtherApps: true)
    }
}

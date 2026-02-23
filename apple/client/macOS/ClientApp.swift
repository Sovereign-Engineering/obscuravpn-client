import Combine
import Network
import NetworkExtension
import OSLog
import Sparkle
import SwiftUI
import SystemExtensions
import UserNotifications

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "App")

@main
class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate, UNUserNotificationCenterDelegate, ObservableObject {
    var primaryWindow: NSWindow!
    var statusItemManager: StatusItemManager?
    var updaterMenuItemSubscription: AnyCancellable?
    var updater: SparkleUpdater?
    var statusItem: NSStatusItem?
    static let FrameAutoSaveName = "root-view"

    static func main() {
        logger.debug("App init")
        // Auto-exit if app is already running
        // Note that this is already rare, but can happen if an installed app is running before running a build from XCode
        if NSWorkspace.shared.runningApplications.filter({
            $0.bundleIdentifier == Bundle.main.bundleIdentifier
        }).count > 1 {
            logger.info("App already running.")
            NSApp.terminate(nil)
            return
        }

        let app = NSApplication.shared
        let delegate = AppDelegate()
        app.delegate = delegate
        app.run()
    }

    func applicationWillFinishLaunching(_ notification: Notification) {
        UNUserNotificationCenter.current().delegate = self
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        // https://stackoverflow.com/a/19890943/7732434
        let event = NSAppleEventManager.shared().currentAppleEvent
        let launchedAsLoginItem = (event?.eventID == kAEOpenApplication && event?.paramDescriptor(forKeyword: keyAEPropData)?.enumCodeValue == keyAELaunchedAsLogInItem)
        logger.log("launched as login item: \(launchedAsLoginItem)")
        if launchedAsLoginItem {
            // Otherwise, the app icon appears in the dock with a black dot with no window
            NSApp.setActivationPolicy(.accessory)
        }
        self.createPrimaryWindow(launchedAsLoginItem: launchedAsLoginItem)
        self.setupMainMenu()
        self.statusItemManager = StatusItemManager()
    }

    @objc func quitApp() {
        NSApp.terminate(nil)
    }

    func openPrimaryWindow() {
        self.primaryWindow.makeKeyAndOrderFront(nil)
        self.primaryWindow.orderFrontRegardless()
    }

    // According to NSWorkspace.shared.menuBarOwningApplication?.localizedName and appWithFocus?.ownsMenuBar
    // obscura owns the menubar and the app with focus owns the menu bar
    // implication: owning the menu bar does not guarantee it shows up...
    // The menubar is blank when Obscura is opened up after switching displays to a monitor (in clamshell)
    // To reproduce the bug, comment out the menu recreation line and follow these instructions.
    // Note that LocalSend also has the same bug, but it cannot even be fixed by switching focus.
    // 1. Close Obscura window (keep it running in status menu)
    // 2. Close macbook lid
    // 3. Connect it to a monitor
    // 4. Open Obscura Manager
    func showPrimaryWindow() {
        NSApp.setActivationPolicy(.regular)
        self.openPrimaryWindow()
        // Fix for blank menubar when switching displays (clamshell mode):
        // Recreate the main menu to ensure it displays correctly
        self.setupMainMenu()
        focusApp()
    }

    // Apple added this method to the template to address a process injection vulnerability related to saving/restoring state
    // https://sector7.computest.nl/post/2022-08-process-injection-breaking-all-macos-security-layers-with-a-single-vulnerability/
    // https://stackoverflow.com/a/77320845/7732434
    // Without this method, log warnings will show up, and the app is apparently vulnerable to compromising SIP
    func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
        return true
    }

    private func createPrimaryWindow(launchedAsLoginItem: Bool) {
        let window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 800, height: 600),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: launchedAsLoginItem
        )
        // must be done before calling `setFrameAutosaveName`
        window.delegate = self
        window.toolbarStyle = .unified
        window.tabbingMode = .disallowed

        // the following enforces view-specific constraints
        let contentView = MainWindowContentView().environmentObject(self)
        let hostingVC = NSHostingController(rootView: contentView)
        hostingVC.sizingOptions = [.minSize]
        window.contentViewController = hostingVC
        // try to restore saved frame
        if !window.setFrameUsingName(Self.FrameAutoSaveName) {
            // without this, the window will off centre on first launch
            window.updateConstraintsIfNeeded()
            window.center()
        }
        window.setFrameAutosaveName(Self.FrameAutoSaveName)
        // maintain previous swift-ui Window behaviour
        window.isReleasedWhenClosed = false
        self.primaryWindow = window

        if !launchedAsLoginItem {
            self.showPrimaryWindow()
        }
    }

    func setupMainMenu() {
        let mainMenu = NSMenu()

        let appMenuItem = NSMenuItem()
        let appMenu = NSMenu()
        appMenuItem.submenu = appMenu

        let aboutItem = NSMenuItem(title: "About Obscura", action: #selector(NSApplication.orderFrontStandardAboutPanel(_:)), keyEquivalent: "")

        let servicesItem = NSMenuItem(title: "Services", action: nil, keyEquivalent: "")
        let servicesMenu = NSMenu()
        servicesItem.submenu = servicesMenu
        NSApp.servicesMenu = servicesMenu

        let hideItem = NSMenuItem(title: "Hide Obscura VPN", action: #selector(NSApplication.hide(_:)), keyEquivalent: "h")

        let hideOthersItem = NSMenuItem(title: "Hide Others", action: #selector(NSApplication.hideOtherApplications(_:)), keyEquivalent: "h")
        hideOthersItem.keyEquivalentModifierMask = [.command, .option]

        let showAllItem = NSMenuItem(title: "Show All", action: #selector(NSApplication.unhideAllApplications(_:)), keyEquivalent: "")

        let closeWindowItem = NSMenuItem(title: "Close Window", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "q")

        appMenu.items = [
            aboutItem,
            NSMenuItem.separator(),
            // Check for Updates will be inserted here when updater is available
            servicesItem,
            NSMenuItem.separator(),
            hideItem,
            hideOthersItem,
            showAllItem,
            NSMenuItem.separator(),
            closeWindowItem,
        ]

        // Check for Updates menu item - will be added when updater is available
        self.updaterMenuItemSubscription = StartupModel.shared.$appState
            .compactMap { $0?.updater }
            .first()
            .receive(on: DispatchQueue.main)
            .sink { [weak self] updater in
                self?.updater = updater
                self?.addCheckForUpdatesMenuItem(to: appMenu)
            }

        let editMenuItem = NSMenuItem()
        let editMenu = NSMenu(title: "Edit")
        editMenuItem.submenu = editMenu

        editMenu.items = [
            NSMenuItem(title: "Undo", action: Selector(("undo:")), keyEquivalent: "z"),
            NSMenuItem(title: "Redo", action: Selector(("redo:")), keyEquivalent: "Z"),
            NSMenuItem.separator(),
            NSMenuItem(title: "Cut", action: #selector(NSText.cut(_:)), keyEquivalent: "x"),
            NSMenuItem(title: "Copy", action: #selector(NSText.copy(_:)), keyEquivalent: "c"),
            NSMenuItem(title: "Paste", action: #selector(NSText.paste(_:)), keyEquivalent: "v"),
            NSMenuItem(title: "Delete", action: #selector(NSText.delete(_:)), keyEquivalent: ""),
            NSMenuItem(title: "Select All", action: #selector(NSText.selectAll(_:)), keyEquivalent: "a"),
            NSMenuItem.separator(),
        ]

        let viewMenuItem = NSMenuItem()
        let viewMenu = NSMenu(title: "View")
        viewMenuItem.submenu = viewMenu

        let fullScreenItem = NSMenuItem(title: "Enter Full Screen", action: #selector(NSWindow.toggleFullScreen(_:)), keyEquivalent: "f")
        fullScreenItem.keyEquivalentModifierMask = [.function]

        viewMenu.items = [
            fullScreenItem,
        ]

        let windowMenuItem = NSMenuItem()
        let windowMenu = NSMenu(title: "Window")
        NSApp.windowsMenu = windowMenu
        windowMenuItem.submenu = windowMenu

        let closeItem = NSMenuItem(title: "Close", action: #selector(NSWindow.performClose(_:)), keyEquivalent: "w")
        let closeAllItem = NSMenuItem(title: "Close All", action: Selector(("closeAll:")), keyEquivalent: "w")
        closeAllItem.keyEquivalentModifierMask = [.command, .option]
        closeAllItem.isAlternate = true

        let minimizeItem = NSMenuItem(title: "Minimize", action: #selector(NSWindow.miniaturize(_:)), keyEquivalent: "m")
        let minimizeAllItem = NSMenuItem(title: "Minimize All", action: #selector(NSWindow.miniaturize(_:)), keyEquivalent: "m")
        minimizeAllItem.keyEquivalentModifierMask = [.command, .option]
        minimizeAllItem.isAlternate = true

        let zoomItem = NSMenuItem(title: "Zoom", action: #selector(NSWindow.zoom(_:)), keyEquivalent: "")
        let zoomAllItem = NSMenuItem(title: "Zoom All", action: #selector(NSWindow.zoom(_:)), keyEquivalent: "")
        zoomAllItem.keyEquivalentModifierMask = [.option]
        zoomAllItem.isAlternate = true

        // https://github.com/avaidyam/Parrot/tree/6cf7ba419176c386ed8f18e838690a7272fe57ee/Parrot
        windowMenu.items = [
            closeItem,
            closeAllItem,
            minimizeItem,
            minimizeAllItem,
            zoomItem,
            zoomAllItem,
            NSMenuItem.separator(),
            NSMenuItem(
                title: "Bring All to Front", action: #selector(NSApplication.arrangeInFront(_:)),
                keyEquivalent: ""
            ),
        ]

        let helpMenuItem = NSMenuItem()
        let helpMenu = NSMenu(title: "Help")
        helpMenuItem.submenu = helpMenu
        NSApp.helpMenu = helpMenu

        mainMenu.items = [
            appMenuItem,
            editMenuItem,
            viewMenuItem,
            windowMenuItem,
            helpMenuItem,
        ]

        NSApp.mainMenu = mainMenu
    }

    private func addCheckForUpdatesMenuItem(to menu: NSMenu) {
        let checkForUpdatesItem = NSMenuItem(
            title: "Check for Updates…", action: #selector(self.checkForUpdates), keyEquivalent: ""
        )
        checkForUpdatesItem.target = self
        menu.insertItem(checkForUpdatesItem, at: 2)
        menu.insertItem(NSMenuItem.separator(), at: 3)
    }

    @objc private func checkForUpdates() {
        self.updater?.showUpdaterIfNeeded()
    }

    // We do not want to depend on applicationShouldTerminateAfterLastWindowClosed,
    // because it can be triggered for a variety of reasons versus triggering for exactly what we want

    // Based on Carl's initial debugging, it was determined:
    // On macOS, when a menu item is highlighted, there is a callback to unhighlight the menu item.
    // If the app is set to accessory before the callback runs, the callback is unable to unhighlight the menu item (for whatever reason).
    // This results in a pre-highlight/stuck state.
    // Recreating the main menu upon opening the window alleviates the need to carefully wait to set the activation policy.
    func windowWillClose(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
    }

    func applicationShouldHandleReopen(_ sender: NSApplication, hasVisibleWindows: Bool) -> Bool {
        logger.debug("from applicationShouldHandleReopen. hasVisibleWindows = \(hasVisibleWindows)")

        if NSApp.activationPolicy() == .regular {
            self.openPrimaryWindow()
            return true
        }

        NSApp.setActivationPolicy(.regular)

        if #available(macOS 14.0, *) {
            self.openPrimaryWindow()
            return true
        }

        // On macos ventura or earlier, without this workaround, if the user
        // reopens the App using Finder while the App is already running, the
        // App Menu (the left side) becomes completely frozen and unusable
        // (even the  one)
        /// more info here:
        // https://linear.app/soveng/issue/OBS-175/no-obscura-vpn-in-menu-bar-dock-or-app-switcher-when-application-is#comment-2ecf3e57
        NSRunningApplication.runningApplications(withBundleIdentifier: "com.apple.systemuiserver")
            .first!.activate(options: [])
        self.openPrimaryWindow()
        NSApp.activate(ignoringOtherApps: true)

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

struct MainWindowContentView: View {
    @ObservedObject var startupModel = StartupModel.shared
    @EnvironmentObject var appDelegate: AppDelegate

    var body: some View {
        Group {
            if let appState = self.startupModel.appState {
                ContentView(appState: appState)
                    .frame(minWidth: 700, minHeight: 525)
            } else {
                StartupView()
                    .frame(minWidth: 800, minHeight: 525)
            }
        }
        .preferredColorScheme(self.startupModel.selectedAppearance.colorScheme)
    }
}

func focusApp() {
    // When opening the app from status menu via
    // the URLs, NSApp.activate() does not cause troubles
    // regarding focus. However, just to be safe regarding edge cases and users, we want to continue
    // using `ignoringOtherApps: true` until it is removed
    if #available(macOS 26.4, *) {
        NSApp.activate()
    } else {
        NSApp.activate(ignoringOtherApps: true)
    }
}

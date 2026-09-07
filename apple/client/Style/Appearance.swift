import Foundation
#if os(macOS)
    import AppKit
#endif
import SwiftUI

enum AppAppearance: String, Codable {
    case dark
    case light
    case auto

    /// The persisted preference, as read by `StartupModel.selectedAppearance`'s `@AppStorage`.
    /// Unlike that property this is not `@MainActor`, so `OsStatus` can read it on any thread.
    static var selected: AppAppearance {
        guard let raw = UserDefaults.standard.string(forKey: UserDefaultKeys.SelectedAppearance) else { return .auto }
        return AppAppearance(rawValue: raw) ?? .auto
    }

    var colorScheme: ColorScheme? {
        switch self {
        case .dark:
            return .dark
        case .light:
            return .light
        case .auto:
            return nil
        }
    }

    #if os(macOS)
        // Set on the window instead of using SwiftUI `preferredColorScheme`: reverting that to nil (auto)
        // leaves the WKWebView pinned to the last explicit scheme; WebKit doesn't learn of auto.
        // `NSApp.appearance` has the same defect.
        // Verified on macOS 26.5.
        var nsAppearance: NSAppearance? {
            switch self {
            case .dark:
                return NSAppearance(named: .darkAqua)
            case .light:
                return NSAppearance(named: .aqua)
            case .auto:
                return nil
            }
        }
    #endif
}

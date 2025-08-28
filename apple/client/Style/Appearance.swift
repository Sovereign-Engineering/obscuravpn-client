#if os(macOS)
    import AppKit
#else
    import UIKit
#endif
import Foundation

enum ColorScheme: String, Codable {
    case dark
    case light
    case auto
}

#if os(macOS)
    func setAppearance(colorScheme: ColorScheme) {
        switch colorScheme {
        case .dark:
            NSApp.appearance = NSAppearance(named: .darkAqua)
        case .light:
            NSApp.appearance = NSAppearance(named: .aqua)
        case .auto:
            NSApp.appearance = nil
        }
    }
#else
    func setAppearance(colorScheme: ColorScheme) {
        var userInterfaceStyle: UIUserInterfaceStyle = .unspecified
        switch colorScheme {
        case .dark:
            userInterfaceStyle = .dark
        case .light:
            userInterfaceStyle = .light
        case .auto:
            userInterfaceStyle = .unspecified
        }
        if let windowScene = UIApplication.shared.connectedScenes.first as? UIWindowScene {
            windowScene.windows.first?.overrideUserInterfaceStyle = userInterfaceStyle
        }
    }
#endif

import SwiftUI

enum AppAppearance: String, Codable {
    case dark
    case light
    case auto

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
}

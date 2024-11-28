import Foundation

enum UserDefaultKeys {
    static let LoginItemRegistered = "LoginItemRegistered"
    static let allKeys = [LoginItemRegistered]
}

enum URLs {
    static let SystemExtensionHelp = URL(string: "https://support.apple.com/en-ca/120363")!
    static let PrivacySecurityExtensionSettings = URL(string: "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Security")!
    static let ExtensionSettings = URL(string: "x-apple.systempreferences:com.apple.LoginItems-Settings.extension?ExtensionItems")!
    static let NetworkSettings = URL(string: "x-apple.systempreferences:com.apple.NetworkExtensionSettingsUI.NESettingsUIExtension")!
}

enum WindowIds {
    static let RootWindowId = "root-view"
}

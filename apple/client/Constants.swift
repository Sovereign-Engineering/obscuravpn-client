import Foundation

enum UserDefaultKeys {
    static let LoginItemRegistered = "LoginItemRegistered"
    static let Appearance = "Appearance"
    static let allKeys = [LoginItemRegistered, Appearance]
}

enum URLs {
    static let SystemExtensionHelp = URL(string: "https://support.apple.com/en-ca/120363")!
    static let PrivacySecurityExtensionSettings = URL(string: "x-apple.systempreferences:com.apple.settings.PrivacySecurity.extension?Security")!
    static let ExtensionSettings = URL(string: "x-apple.systempreferences:com.apple.LoginItems-Settings.extension?ExtensionItems")!
    static let NetworkSettings = URL(string: "x-apple.systempreferences:com.apple.NetworkExtensionSettingsUI.NESettingsUIExtension")!
    // See [Deep Linking](https://soveng.getoutline.com/doc/deep-linking-rhhx0E5oDB)
    static let AppAccountPage = URL(string: "obscuravpn:///account")!
    static let AppLocationPage = URL(string: "obscuravpn:///location")!
}

enum WindowIds {
    static let RootWindowId = "root-view"
}

import Foundation

// The unique build ID.
//
// This is basically a meaningless number, it shouldn't be shown to users. It can just be used to tell if the exact same binary is being used. It is also used for updates as it is a monotonically increasing value.
func buildVersion() -> String {
    Bundle.main.infoDictionary!["CFBundleVersion"] as! String
}

private func obscuraInfoDict() -> [String: Any] {
    Bundle.main.infoDictionary!["Obscura"] as! [String: Any]
}

// This is the main version number.
//
// This number is suitable for showing to the user as it contains just the information needed to usefully describe the version.
//
// In release builds it will be pretty such as v1.23.
//
// In other builds will will be something like `v1.23-3-abcde123` or `v1.23-6-a1b2c3-dirty`.
func sourceVersion() -> String {
    return obscuraInfoDict()["ObscuraSourceVersion"] as! String
}

// The source commit ID.
//
// This will be a full commit ID, suffixed with -dirty if the working directory was not comitted.
//
// It generally shouldn't be shown to users, use `sourceVersion` instead.
func sourceId() -> String {
    return obscuraInfoDict()["ObscuraSourceId"] as! String
}

let useSystemExtension = true

func extensionBundleID() -> String {
    switch useSystemExtension {
    case true: systemNetworkExtensionBundleID()
    case false: appNetworkExtensionBundleID()
    }
}

func extensionBundle() -> Bundle {
    let url = Bundle.main.bundleURL
        .appending(path: "Contents/Library/SystemExtensions/")
        .appending(component: "\(extensionBundleID()).systemextension")

    return Bundle(url: url)!
}

private func systemNetworkExtensionBundleID() -> String {
    return obscuraInfoDict()["SystemNetworkExtensionBundleIdentifier"] as! String
}

private func appNetworkExtensionBundleID() -> String {
    return obscuraInfoDict()["AppNetworkExtensionBundleIdentifier"] as! String
}

func appGroupID() -> String {
    return obscuraInfoDict()["AppGroupIdentifier"] as! String
}

func configDir() -> String {
    return "/Library/Application Support/obscura-vpn/system-network-extension/"
}

func oldConfigDir() -> String {
    FileManager.default.containerURL(forSecurityApplicationGroupIdentifier: appGroupID())!
        .appending(components: "Library", "Application Support", "obscuravpn", directoryHint: .isDirectory).path(percentEncoded: false)
}

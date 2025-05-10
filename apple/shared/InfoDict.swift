import Foundation
import UniformTypeIdentifiers

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

func extensionBundle() -> Bundle {
    let url = Bundle.main.bundleURL
        .appending(path: "Contents/Library/SystemExtensions/")
        .appending(component: "\(networkExtensionBundleID()).systemextension")

    return Bundle(url: url)!
}

// The correct bundle ID for the client app to connect to based on the build configuration
public func networkExtensionBundleID() -> String {
    return obscuraInfoDict()["OBSCURA_NETWORK_EXTENSION_BUNDLE_ID"] as! String
}

func appGroupID() -> String {
    return obscuraInfoDict()["AppGroupIdentifier"] as! String
}

func configDir() -> String {
    #if os(macOS)
        return "/Library/Application Support/obscura-vpn/system-network-extension/"
    #else
        return URL.libraryDirectory.appendingPathComponent("obscura", conformingTo: UTType.folder).path(percentEncoded: false)
    #endif
}

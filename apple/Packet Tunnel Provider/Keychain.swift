import Foundation
import Security
import OSLog

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "keychain")
private let wgSecretKeyquery: [String: Any] = [
    kSecClass as String: kSecClassGenericPassword,
    kSecAttrService as String: "obscura",
    kSecAttrAccount as String: "wireguard-sk",
]

func keychainSetWgSecretKey(_ sk: Data) -> Bool {
    SecItemDelete(wgSecretKeyquery as CFDictionary)
    var insert = wgSecretKeyquery;
    insert[kSecValueData as String] = sk
    let insertStatus = SecItemAdd(insert as CFDictionary, nil)
    switch insertStatus {
    case errSecSuccess:
        logger.log("keychain item inserted")
        return true
    default:
        logger.error("error inserting keychain item: \(insertStatus, privacy: .public)")
        return false
    }
}

func keychainGetWgSecretKey() -> Data? {
    var get = wgSecretKeyquery
    get[kSecMatchLimit as String] = kSecMatchLimitOne
    get[kSecReturnData as String] = kCFBooleanTrue

    var item: CFTypeRef?
    let status = SecItemCopyMatching(get as CFDictionary, &item)
    switch status {
    case errSecSuccess:
        logger.log("keychain item found")
    case errSecItemNotFound:
        logger.log("keychain item not found")
    default:
        logger.error("error getting keychain item: \(status, privacy: .public)")
        return .none
    }
    
    guard let data = item as? NSData else {
        logger.error("got unexpected result format from keychain")
        return .none
    }
    
    return data as Data
}

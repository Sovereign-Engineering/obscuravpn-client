import Foundation
import Security

private let wgSecretKeyquery: [String: Any] = [
    kSecClass as String: kSecClassGenericPassword,
    kSecAttrService as String: "obscura",
    kSecAttrAccount as String: "wireguard-sk",
]

func keychainSetWgSecretKey(_ sk: Data) -> Bool {
    SecItemDelete(wgSecretKeyquery as CFDictionary)
    var insert = wgSecretKeyquery
    insert[kSecValueData as String] = sk
    insert[kSecAttrAccessible as String] = kSecAttrAccessibleAlwaysThisDeviceOnly
    let insertStatus = SecItemAdd(insert as CFDictionary, nil)
    switch insertStatus {
    case errSecSuccess:
        ffiLog(.Info, "keychain item inserted")
        return true
    default:
        ffiLog(.Error, "error inserting keychain item: \(insertStatus)")
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
        ffiLog(.Info, "keychain item found")
    case errSecItemNotFound:
        ffiLog(.Info, "keychain item not found")
    default:
        ffiLog(.Error, "error getting keychain item: \(status)")
        return .none
    }

    guard let data = item as? NSData else {
        ffiLog(.Error, "got unexpected result format from keychain")
        return .none
    }

    return data as Data
}

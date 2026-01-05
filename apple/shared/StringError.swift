import Foundation

// Required to use `String` as `.failure` variant in `Result`
extension String: LocalizedError {
    public var errorDescription: String? { return self }
}

// Define "ipcError-$" in webUI i18n files
let errorCodeOther: String = "other"
let errorCodeTunnelInactive: String = "tunnelInactive"
let errorCodeUpdaterCheck: String = "updaterFailedToCheck"
let errorCodeUpdaterInstall: String = "updaterFailedToStartInstall"
let errorUnsupportedOnOS: String = "errorUnsupportedOnOS"
let errorFailedToAssociateAccount: String = "failedToAssociateAccount"
let errorPurchaseFailed: String = "purchaseFailed"

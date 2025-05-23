import Foundation

// Required to use `String` as `.failure` variant in `Result`
extension String: LocalizedError {
    public var errorDescription: String? { return self }
}

let errorCodeOther: String = "other"
let errorCodeTunnelInactive: String = "tunnelInactive"
let errorCodeUpdaterCheck: String = "updaterFailedToCheck"
let errorCodeUpdaterInstall: String = "updaterFailedToStartInstall"
let errorUnsupportedOnOS: String = "errorUnsupportedOnOS"

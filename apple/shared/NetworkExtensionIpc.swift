import Foundation
import NetworkExtension
import OSLog

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "network extension ipc")

enum NeFfiJsonResult: Codable {
    case ok_json(String)
    case error(String)
}

struct TunnelArgs: Codable {
    var exit: String?
}

struct NeStatus: Codable, Equatable {
    var version: UUID
    var vpnStatus: NeVpnStatus
    var accountId: String?
    var inNewAccountFlow: Bool
    var pinnedLocations: [PinnedLocation]
    var lastChosenExit: String?
    var apiUrl: String
    var account: AccountStatus?

    static func == (left: NeStatus, right: NeStatus) -> Bool {
        return left.version == right.version
    }
}

struct PinnedLocation: Codable, Equatable {
    var country_code: String
    var city_code: String
    var pinned_at: Int64
}

enum NeVpnStatus: Codable {
    case connecting
    case connected(exit: ExitInfo, client_public_key: String, exit_public_key: String)
    case reconnecting(exit: ExitInfo, err: String?)
    case disconnected
}

struct ExitInfo: Codable {
    var id: String
    var country_code: String
    var city_name: String
}

// We must use NSError to communicate errors via startTunnel.
// This defines an error domain and related methods for our Rust `ConnectErrorCode`.
extension NSError {
    convenience init(connectErrorCode: String) {
        self.init(domain: connectErrorDomain, code: 0, userInfo: [variantKey: connectErrorCode])
    }

    func connectErrorCode() -> String? {
        if self.domain == connectErrorDomain {
            guard let value = self.userInfo[variantKey] else {
                logger.error("domain is \(connectErrorDomain) no \(variantKey) key on userInfo")
                return nil
            }
            guard let connectErrorCode = value as? String else {
                logger.error("domain is \(connectErrorDomain), but userInfo.\(variantKey) is not a String")
                return nil
            }
            return connectErrorCode
        }
        return nil
    }
}

private let connectErrorDomain = "net.obscura.ConnectErrorCode"
private let variantKey = "variant"

extension NEVPNStatus: CustomStringConvertible {
    public var description: String {
        return switch self {
        case .invalid:
            "invalid"
        case .disconnected:
            "disconnected"
        case .connecting:
            "connecting"
        case .connected:
            "connected"
        case .reasserting:
            "reasserting"
        case .disconnecting:
            "disconnecting"
        @unknown default:
            "unknown (rawValue: \(self.rawValue))"
        }
    }
}

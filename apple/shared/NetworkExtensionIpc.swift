import Foundation
import NetworkExtension
import OSLog

private let logger = Logger(subsystem: Bundle.main.bundleIdentifier!, category: "network extension ipc")

// See ../../rustlib/src/manager_cmd.rs
enum NeManagerCmdResult: Codable {
    case ok_json(String)
    case error(String)
}

// See ../../rustlib/src/manager_cmd.rs
enum NeManagerCmd: Codable {
    case apiAppleAssociateAccount(appTransactionJws: String)
    case getDebugInfo
    case apiAppleCreateAppAccountToken
    case apiApplePollSubscription(originalTransactionId: String)
    case apiGetAccountInfo
    case getLogDir
    case getStatus(knownVersion: UUID?)
    case getTrafficStats
    case ping
    case setTunnelArgs(args: TunnelArgs?, allowActivation: Bool = false)
    case login(accountId: String, validate: Bool)
    case getExitList(knownVersion: String?)
    case refreshExitList(freshness: TimeInterval)
}

// See ../../rustlib/src/manager.rs
struct TunnelArgs: Codable {
    var exit: ExitSelector
}

// See ../../rustlib/src/manager.rs
enum ExitSelector: Codable {
    case any
    case exit(id: String)
    case country(country_code: String)
    case city(
        country_code: String,
        city_code: String
    )
}

struct NeStatus: Codable, Equatable {
    var version: UUID
    var vpnStatus: NeVpnStatus
    var accountId: String?
    var inNewAccountFlow: Bool
    var pinnedLocations: [PinnedLocation]
    var lastChosenExit: ExitSelector
    var lastExit: ExitSelector
    var apiUrl: String
    var account: AccountStatus?
    var autoConnect: Bool

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
    case connecting(tunnelArgs: TunnelArgs, connectError: String?, reconnecting: Bool)
    case connected(tunnelArgs: TunnelArgs, exit: ExitInfo, networkConfig: NetworkConfig, exitPublicKey: String, clientPublicKey: String, transport: TransportKind)
    case disconnected
}

struct ExitInfo: Codable {
    var id: String
    var country_code: String
    var city_name: String
    var city_code: String
}

enum TransportKind: String, Codable, Equatable {
    case quic
    case tcpTls
}

// Keep synchronized with rustlib/src/apple/network_config.rs
struct NetworkConfig: Codable, CustomStringConvertible, Equatable {
    var description: String {
        return "ipv4: \(self.ipv4), dns: \(self.dns), ipv6: \(self.ipv6)"
    }

    var ipv4: String
    var dns: [String]
    var ipv6: String
    var mtu: UInt16
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

import Foundation
import NetworkExtension

extension NEVPNStatus: Encodable {
    public func encode(to encoder: any Encoder) throws {
        var container = encoder.singleValueContainer()
        switch self {
        case .invalid:
            try container.encode("invalid")
        case .disconnected:
            try container.encode("disconnected")
        case .connecting:
            try container.encode("connecting")
        case .connected:
            try container.encode("connected")
        case .reasserting:
            try container.encode("reasserting")
        case .disconnecting:
            try container.encode("disconnecting")
        @unknown default:
            try container.encode("unknown")
        }
    }
}

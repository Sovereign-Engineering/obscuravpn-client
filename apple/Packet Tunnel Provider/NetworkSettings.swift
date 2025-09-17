import Foundation
import NetworkExtension

extension NEPacketTunnelNetworkSettings {
    static func build(_ ffiNetworkConfig: NetworkConfig) -> NEPacketTunnelNetworkSettings {
        let networkSettings = NEPacketTunnelNetworkSettings(tunnelRemoteAddress: "127.0.0.1")

        networkSettings.mtu = ffiNetworkConfig.mtu as NSNumber

        let ipv4Settings = NEIPv4Settings(
            addresses: [ffiNetworkConfig.ipv4],
            subnetMasks: ["255.255.255.255"]
        )
        ipv4Settings.includedRoutes = [NEIPv4Route.default()]
        networkSettings.ipv4Settings = ipv4Settings

        let selfIpv6Parts = ffiNetworkConfig.ipv6.split(separator: "/", maxSplits: 1)
        let selfIpv6Addr = String(selfIpv6Parts[0])
        let selfIpv6Prefix = UInt8(selfIpv6Parts[1])!

        let ipv6Settings = NEIPv6Settings(
            addresses: [selfIpv6Addr],

            // If a too-small network is used we won't be granted the default IPv6 route. So cap the prefix length. This shouldn't be in issue for us since the IP is always a private IP that gets NATed. If that ever changes we will likely end up with a bigger prefix anyways, but either way that is a problem for the future.
            //
            // Testing has shown that anything smaller than a /125 network won't work on macOS.
            //
            // wireguard-apple suggests that a /120 may be required on iOS: https://github.com/WireGuard/wireguard-apple/blob/af58bfcb00e7ebdd0c0f48d2f15df17ab3b2b8d7/WireGuard/WireGuardNetworkExtension/PacketTunnelSettingsGenerator.swift#L165-L170
            networkPrefixLengths: [NSNumber(value: min(selfIpv6Prefix, 125))]
        )
        ipv6Settings.includedRoutes = [NEIPv6Route.default()]
        networkSettings.ipv6Settings = ipv6Settings

        let dns_settings = NEDNSSettings(servers: ffiNetworkConfig.dns)

        // TODO: Is this necessary anymore?
        dns_settings.matchDomains = [""] // Match everything. https://developer.apple.com/documentation/networkextension/nednssettings/1406537-matchdomains (2024-07-11)

        networkSettings.dnsSettings = dns_settings

        return networkSettings
    }
}

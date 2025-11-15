import Foundation
import NetworkExtension

extension NEPacketTunnelNetworkSettings {
    static func build(_ osNetworkConfig: OsNetworkConfig) -> NEPacketTunnelNetworkSettings {
        let networkSettings = NEPacketTunnelNetworkSettings(tunnelRemoteAddress: "127.0.0.1")

        networkSettings.mtu = osNetworkConfig.tunnelNetworkConfig.mtu as NSNumber

        let ipv4Settings = NEIPv4Settings(
            addresses: [osNetworkConfig.tunnelNetworkConfig.ipv4],
            subnetMasks: ["255.255.255.255"]
        )
        ipv4Settings.includedRoutes = [NEIPv4Route.default()]
        networkSettings.ipv4Settings = ipv4Settings

        let selfIpv6Parts = osNetworkConfig.tunnelNetworkConfig.ipv6.split(separator: "/", maxSplits: 1)
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

        let dns_settings = NEDNSSettings(servers: osNetworkConfig.tunnelNetworkConfig.dns)

        if osNetworkConfig.useSystemDns {
            // Contrary to apple documentation this is not ignored if the VPN tunnel is the default route and allows us to fall back on configured DNS profiles. (https://developer.apple.com/documentation/networkextension/nednssettings/matchdomains (2025-11-15))
            dns_settings.matchDomains = ["invalid.obscura.net"]
        } else {
            // This is not necessary to match everything if the VPN tunnel is the default route, but is harmless either way. (https://developer.apple.com/documentation/networkextension/nednssettings/matchdomains (2025-11-15))
            dns_settings.matchDomains = [""]
        }

        networkSettings.dnsSettings = dns_settings

        return networkSettings
    }
}

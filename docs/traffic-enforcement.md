# Traffic enforcement

This document describes what traffic we allow while the user wants to be connected. The primary concerns are:
- Traffic leaves the machine only through the tunnel.
- Tunnel flows cannot be impersonated from outside.
- The exit cannot initiate flows.

Enforcement starts when the target state becomes connected, which includes connecting or reconnecting transient states.

## Basics by platform

- **Linux.** Nftables tables and chains enforce only desired traffic flows. FWMARK on the service's own relay and API traffic allows bypassing this firewall. Enforcement is largely independent of routing.
- **Windows.** Routing only. Two half-default routes (per address family) beat the system default route, but routes more specific than /1 win.
- **Apple.** Routing only, unless strict leak prevention is opted into, which enables [`includeAllNetworks`](https://developer.apple.com/documentation/networkextension/nevpnprotocol/includeallnetworks) mode, blocking all traffic outside the tunnel.
- **Android.** Routing only, unless the user enables "Always-on VPN" and "Block connections without VPN" in system settings.

## Concerns

In the following we list specific concerns and how/if they are addressed on each platform. The focus is on enforcing the desired limitations not happy case routing.

### Egress outside the tunnel

Only deliberate exceptions may leave on an interface other than the tun. Linux enforces this by dropping all packets that are not explicitly allowed via a nftables postrouting chain. Apple and Android expose configuration of the OS-implemented enforcement. Windows currently implements no enforcement beyond setting /1 routes for each half of the address space.

| Exception | Linux | Windows | Apple | Android |
| --- | --- | --- | --- | --- |
| Loopback | allowed | allowed | allowed | allowed |
| Service traffic | allowed | allowed | allowed | allowed |
| Traffic via explicitly bound device/address | denied | allowed | default-allowed | denied |
| Traffic into our tun | allowed | allowed | allowed | allowed |
| Local network destination ranges | default-allowed | allowed | default-allowed | default-allowed |
| DHCP and IPv6 neighbor discovery | allowed | allowed | allowed | allowed |
| [Forwarded flows](#exception-forwarded-flows) | allowed | allowed | allowed | NA |
| Traffic into WireGuard devices | default-allowed | allowed | default-allowed | NA |
| Traffic into Tailscale device | default-allowed | allowed | default-allowed | NA |
| Tailscale service traffic bypass | default-allowed | NA | NA | NA |

#### Exception: Forwarded flows

Allows reply traffic arriving through the tun for flows that originate from other network devices. E.g. flows initiated by a Docker container via the docker0 device or local network traffic if the machine is configured as a gateway.

On Linux we also accept ICMP and ICMPv6 packets that conntrack marks as related to allowed flows, so locally generated ICMP errors are delivered as well (e.g. for path MTU discovery).

### Routing table attacks from the local network

DHCP servers can push routes more specific than a default (e.g. [TunnelVision, CVE-2024-3661](https://nvd.nist.gov/vuln/detail/CVE-2024-3661)).

- **Linux.** The packet reaches the physical interface without the fwmark and is dropped, unless it is a LAN destination and local network access is enabled.
- **Windows.** A route more specific than a /1 wins.
- **Apple.** Enabling strict leak prevention prevents egress via physical device.
- **Android.** The OS prioritizes VPN route configuration.

### Unsolicited traffic from the exit

The exit can inject arbitrary packets into the tunnel. On Linux packets from the tun are only accepted if conntrack recognizes them as replies of an existing flow.

### Unsolicited traffic from the local network

Regardless of whether their replies may be dropped, non-reply incoming packets may trigger application level behavior (one-shot UDP datagrams, TCP Fast Open data, QUIC 0-RTT).

Not handled on Windows, but Windows Defender Firewall does not allow inbound flows by default.

On Linux packets for this host are dropped unless they are replies to existing flows or link maintenance. Local network, WireGuard and Tailscale exceptions apply the same as they do for egress. Forwarded traffic is not affected.


### Tunnel traffic injection from the local network

On weak host model systems (all except Windows) packets addressed to the tunnel address are delivered as if they came through the tunnel. This allows LAN peers to probe for the tunnel address and inject packets into tunneled connections ([CVE-2019-14899](https://nvd.nist.gov/vuln/detail/CVE-2019-14899)).

On Linux we drop packets for the tunnel address arriving on any interface other than the tun or loopback.

### Discovering the tunnel address by ARP

Linux answers ARP requests for local IPv4 addresses on any interface. Since tunnel addresses are stable until the WireGuard key is rotated, this would identify a machine across networks. To prevent this we drop ARP requests for the tunnel address on Linux.


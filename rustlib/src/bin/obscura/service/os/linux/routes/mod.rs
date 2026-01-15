pub mod netlink;

use ipnetwork::{IpNetwork, Ipv4Network, Ipv6Network};
use std::net::{Ipv4Addr, Ipv6Addr};

/// The individual routes cover half of the respective address space, which gives them priority over the default route without replacing it. We don't want to replace the default route, because:
/// - We use the default route for preferred network interface discovery
/// - We wouldn't know what the set it to when the tunnel is disabled
/// - Network management services like network manager tend to overwrite it.
pub const ROUTES: [IpNetwork; 4] = [
    IpNetwork::V4(Ipv4Network::new_checked(Ipv4Addr::new(000, 0, 0, 0), 1).unwrap()),
    IpNetwork::V4(Ipv4Network::new_checked(Ipv4Addr::new(128, 0, 0, 0), 1).unwrap()),
    IpNetwork::V6(Ipv6Network::new_checked(Ipv6Addr::new(0x0000, 0, 0, 0, 0, 0, 0, 0), 1).unwrap()),
    IpNetwork::V6(Ipv6Network::new_checked(Ipv6Addr::new(0x8000, 0, 0, 0, 0, 0, 0, 0), 1).unwrap()),
];

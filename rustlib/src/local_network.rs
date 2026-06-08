use ipnetwork::{IpNetwork, Ipv4Network, Ipv6Network};
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

const ALL_V4: Ipv4Network = Ipv4Network::new_checked(Ipv4Addr::UNSPECIFIED, 0).unwrap();
const ALL_V6: Ipv6Network = Ipv6Network::new_checked(Ipv6Addr::UNSPECIFIED, 0).unwrap();

const LAN_V4: [Ipv4Network; 7] = [
    Ipv4Network::new_checked(Ipv4Addr::new(10, 0, 0, 0), 8).unwrap(), // private (RFC 1918)
    Ipv4Network::new_checked(Ipv4Addr::new(172, 16, 0, 0), 12).unwrap(), // private (RFC 1918)
    Ipv4Network::new_checked(Ipv4Addr::new(192, 168, 0, 0), 16).unwrap(), // private (RFC 1918)
    Ipv4Network::new_checked(Ipv4Addr::new(169, 254, 0, 0), 16).unwrap(), // link-local (RFC 3927)
    Ipv4Network::new_checked(Ipv4Addr::new(255, 255, 255, 255), 32).unwrap(), // limited broadcast
    Ipv4Network::new_checked(Ipv4Addr::new(224, 0, 0, 0), 24).unwrap(), // link-local multicast (RFC 5771)
    Ipv4Network::new_checked(Ipv4Addr::new(239, 0, 0, 0), 8).unwrap(), // administratively scoped multicast (RFC 2365)
];

const LAN_V6: [Ipv6Network; 7] = [
    Ipv6Network::new_checked(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 0), 10).unwrap(), // link-local (RFC 4291)
    Ipv6Network::new_checked(Ipv6Addr::new(0xfc00, 0, 0, 0, 0, 0, 0, 0), 7).unwrap(),  // unique local / ULA (RFC 4193)
    Ipv6Network::new_checked(Ipv6Addr::new(0xff01, 0, 0, 0, 0, 0, 0, 0), 16).unwrap(), // interface-local multicast
    Ipv6Network::new_checked(Ipv6Addr::new(0xff02, 0, 0, 0, 0, 0, 0, 0), 16).unwrap(), // link-local multicast
    Ipv6Network::new_checked(Ipv6Addr::new(0xff03, 0, 0, 0, 0, 0, 0, 0), 16).unwrap(), // realm-local multicast
    Ipv6Network::new_checked(Ipv6Addr::new(0xff04, 0, 0, 0, 0, 0, 0, 0), 16).unwrap(), // admin-local multicast
    Ipv6Network::new_checked(Ipv6Addr::new(0xff05, 0, 0, 0, 0, 0, 0, 0), 16).unwrap(), // site-local multicast
];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Route {
    pub address: IpAddr,
    pub prefix: u8,
}

impl From<IpNetwork> for Route {
    fn from(network: IpNetwork) -> Self {
        Self { address: network.network(), prefix: network.prefix() }
    }
}

pub fn tunnel_routes(dns_servers: &[IpAddr], allow_local_network_access: bool) -> Vec<Route> {
    if !allow_local_network_access {
        return vec![IpNetwork::V4(ALL_V4).into(), IpNetwork::V6(ALL_V6).into()];
    }

    let mut dns_v4 = Vec::new();
    let mut dns_v6 = Vec::new();
    for server in dns_servers {
        match server {
            IpAddr::V4(address) => dns_v4.push(Ipv4Network::from(*address)),
            IpAddr::V6(address) => dns_v6.push(Ipv6Network::from(*address)),
        }
    }

    let local_v4 = dns_v4.into_iter().fold(LAN_V4.to_vec(), subtract);
    let local_v6 = dns_v6.into_iter().fold(LAN_V6.to_vec(), subtract);

    let v4 = local_v4.into_iter().fold(vec![ALL_V4], subtract).into_iter().map(IpNetwork::V4);
    let v6 = local_v6.into_iter().fold(vec![ALL_V6], subtract).into_iter().map(IpNetwork::V6);
    v4.chain(v6).map(Route::from).collect()
}

trait Network: Copy {
    /// Whether `self` is a supernet of (or equal to) `other`.
    fn covers(self, other: Self) -> bool;
    /// The two sub-networks with one additional prefix bit.
    fn halves(self) -> [Self; 2];
}

fn subtract<N: Network>(mut networks: Vec<N>, cut: N) -> Vec<N> {
    let mut i = 0;
    while i < networks.len() {
        let net = networks[i];
        if cut.covers(net) {
            networks.remove(i);
        } else if net.covers(cut) {
            let [low, high] = net.halves();
            networks[i] = low;
            networks.insert(i + 1, high);
        } else {
            i += 1;
        }
    }
    networks
}

impl Network for Ipv4Network {
    fn covers(self, other: Self) -> bool {
        self.prefix() <= other.prefix() && self.contains(other.network())
    }

    fn halves(self) -> [Self; 2] {
        let prefix = self.prefix() + 1;
        let base = u32::from(self.network());
        let high_bit = 1u32 << (32 - u32::from(prefix));
        [
            Ipv4Network::new_checked(Ipv4Addr::from(base), prefix).unwrap(),
            Ipv4Network::new_checked(Ipv4Addr::from(base | high_bit), prefix).unwrap(),
        ]
    }
}

impl Network for Ipv6Network {
    fn covers(self, other: Self) -> bool {
        self.prefix() <= other.prefix() && self.contains(other.network())
    }

    fn halves(self) -> [Self; 2] {
        let prefix = self.prefix() + 1;
        let base = u128::from(self.network());
        let high_bit = 1u128 << (128 - u32::from(prefix));
        [
            Ipv6Network::new_checked(Ipv6Addr::from(base), prefix).unwrap(),
            Ipv6Network::new_checked(Ipv6Addr::from(base | high_bit), prefix).unwrap(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ipnetwork::IpNetwork;

    fn covered(routes: &[Route], address: &str) -> bool {
        let address: IpAddr = address.parse().unwrap();
        routes
            .iter()
            .any(|route| IpNetwork::new(route.address, route.prefix).unwrap().contains(address))
    }

    #[test]
    fn tunnel_route_set() {
        let dns: [IpAddr; 2] = ["10.64.0.1".parse().unwrap(), "fc00:bbbb:bbbb:bb01::1".parse().unwrap()];
        let render = |routes: &[Route]| routes.iter().map(|r| format!("{}/{}", r.address, r.prefix)).collect::<Vec<_>>();

        let disabled = tunnel_routes(&dns, false);
        assert_eq!(render(&disabled), ["0.0.0.0/0", "::/0"]);
        for address in ["1.1.1.1", "192.168.1.5", "fe80::1"] {
            assert!(covered(&disabled, address), "{address} should route through the tunnel");
        }

        let enabled = tunnel_routes(&dns, true);
        #[rustfmt::skip]
        assert_eq!(render(&enabled), [
            // IPv4:
            "0.0.0.0/5", "8.0.0.0/7",
            "10.64.0.1/32", // 10.0.0.0/8
            "11.0.0.0/8", "12.0.0.0/6", "16.0.0.0/4", "32.0.0.0/3", "64.0.0.0/2", "128.0.0.0/3", "160.0.0.0/5", "168.0.0.0/8",
            "169.0.0.0/9", "169.128.0.0/10", "169.192.0.0/11", "169.224.0.0/12", "169.240.0.0/13", "169.248.0.0/14", "169.252.0.0/15",
            // 169.254.0.0/16
            "169.255.0.0/16", "170.0.0.0/7", "172.0.0.0/12",
            // 172.16.0.0/12
            "172.32.0.0/11", "172.64.0.0/10", "172.128.0.0/9", "173.0.0.0/8", "174.0.0.0/7", "176.0.0.0/4",
            "192.0.0.0/9", "192.128.0.0/11", "192.160.0.0/13",
            // 192.168.0.0/16
            "192.169.0.0/16", "192.170.0.0/15", "192.172.0.0/14", "192.176.0.0/12", "192.192.0.0/10",
            "193.0.0.0/8", "194.0.0.0/7", "196.0.0.0/6", "200.0.0.0/5", "208.0.0.0/4",
            // 224.0.0.0/24
            "224.0.1.0/24", "224.0.2.0/23", "224.0.4.0/22", "224.0.8.0/21", "224.0.16.0/20", "224.0.32.0/19", "224.0.64.0/18", "224.0.128.0/17",
            "224.1.0.0/16", "224.2.0.0/15", "224.4.0.0/14", "224.8.0.0/13", "224.16.0.0/12", "224.32.0.0/11", "224.64.0.0/10", "224.128.0.0/9",
            "225.0.0.0/8", "226.0.0.0/7", "228.0.0.0/6", "232.0.0.0/6", "236.0.0.0/7", "238.0.0.0/8",
            // 239.0.0.0/8
            "240.0.0.0/5", "248.0.0.0/6", "252.0.0.0/7", "254.0.0.0/8",
            "255.0.0.0/9", "255.128.0.0/10", "255.192.0.0/11", "255.224.0.0/12", "255.240.0.0/13", "255.248.0.0/14", "255.252.0.0/15", "255.254.0.0/16",
            "255.255.0.0/17", "255.255.128.0/18", "255.255.192.0/19", "255.255.224.0/20", "255.255.240.0/21", "255.255.248.0/22", "255.255.252.0/23", "255.255.254.0/24",
            "255.255.255.0/25", "255.255.255.128/26", "255.255.255.192/27", "255.255.255.224/28", "255.255.255.240/29", "255.255.255.248/30", "255.255.255.252/31", "255.255.255.254/32",
            // IPv6:
            "::/1", "8000::/2", "c000::/3", "e000::/4", "f000::/5", "f800::/6",
            "fc00:bbbb:bbbb:bb01::1/128", // fc00::/7
            "fe00::/9",
            // fe80::/10
            "fec0::/10", "ff00::/16",
            // ff01::/16 - ff05::/16
            "ff06::/15", "ff08::/13", "ff10::/12", "ff20::/11", "ff40::/10", "ff80::/9",
        ]);
        for public in ["1.1.1.1", "8.8.8.8", "100.64.0.1", "2606:4700:4700::1111"] {
            assert!(covered(&enabled, public), "{public} should route through the tunnel");
        }
        for local in [
            "192.168.1.5",
            "10.5.5.5",
            "172.16.9.9",
            "169.254.1.1",
            "224.0.0.251",
            "239.255.255.250",
            "fe80::1",
            "fc00::1",
            "ff02::fb",
        ] {
            assert!(!covered(&enabled, local), "{local} should bypass the tunnel");
        }
        for resolver in ["10.64.0.1", "fc00:bbbb:bbbb:bb01::1"] {
            assert!(covered(&enabled, resolver), "{resolver} should stay in the tunnel");
        }
    }

    #[test]
    fn subtract_single_v4() {
        let ten: Ipv4Network = "10.0.0.0/8".parse().unwrap();
        let routes: Vec<String> = subtract(vec![ALL_V4], ten).iter().map(ToString::to_string).collect();
        assert_eq!(
            routes,
            [
                "0.0.0.0/5",
                "8.0.0.0/7",
                "11.0.0.0/8",
                "12.0.0.0/6",
                "16.0.0.0/4",
                "32.0.0.0/3",
                "64.0.0.0/2",
                "128.0.0.0/1"
            ]
        );
    }
}

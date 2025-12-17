// This file contains constants, which may need to be updated at some point

use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub const DEFAULT_API_URL: &str = "https://v1.api.prod.obscura.net/api";
pub const DNS_CACHE_SEED: &[(&str, &[SocketAddr])] = &[(DEFAULT_API_URL, &[SocketAddr::new(IpAddr::V4(Ipv4Addr::new(66, 42, 95, 12)), 0)])];

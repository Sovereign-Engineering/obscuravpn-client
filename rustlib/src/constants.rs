// This file contains constants, which may need to be updated at some point

use const_format::formatcp;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

pub const DEFAULT_API_DOMAIN: &str = "v1.api.prod.obscura.net";
pub const DEFAULT_API_URL: &str = formatcp!("https://{DEFAULT_API_DOMAIN}/api");
pub const DNS_CACHE_SEED: &[(&str, &[SocketAddr])] = &[(DEFAULT_API_DOMAIN, &[SocketAddr::new(IpAddr::V4(Ipv4Addr::new(66, 42, 95, 12)), 0)])];

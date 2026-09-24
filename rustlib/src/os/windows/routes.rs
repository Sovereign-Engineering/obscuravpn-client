//! Get route and source address of a new unbound socket would use, via `GetBestRoute2`

use crate::int_helper::u32_into_usize;
use serde::Serialize;
use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use windows::Win32::NetworkManagement::IpHelper::{ConvertInterfaceLuidToAlias, GetBestRoute2, MIB_IPFORWARD_ROW2};
use windows::Win32::NetworkManagement::Ndis::{IF_MAX_STRING_SIZE, NET_LUID_LH};
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6, IN_ADDR, IN_ADDR_0, IN6_ADDR, IN6_ADDR_0, SOCKADDR_IN, SOCKADDR_IN6, SOCKADDR_INET};
use windows::core::PWSTR;

#[derive(Debug, Serialize)]
pub struct BestRoute {
    pub source_address: Option<IpAddr>,
    pub destination: Option<IpAddr>,
    pub prefix_length: u8,
    pub interface_index: u32,
    pub interface_alias: Option<String>,
    pub route_metric: u32,
    /// `NL_ROUTE_PROTOCOL`: https://learn.microsoft.com/windows/win32/api/nldef/ne-nldef-nl_route_protocol
    pub protocol: i32,
}

pub fn best_routes(destinations: &[IpAddr]) -> BTreeMap<IpAddr, Result<BestRoute, String>> {
    destinations
        .iter()
        .map(|&destination| (destination, best_route(destination).map_err(|error| error.to_string())))
        .collect()
}

fn best_route(destination: IpAddr) -> windows::core::Result<BestRoute> {
    let destination = sockaddr_inet(destination);
    let mut row = MIB_IPFORWARD_ROW2::default();
    let mut source = SOCKADDR_INET::default();
    // No interface or source constraint, so this is the lookup a new unbound socket gets.
    // SAFETY: `destination` is a fully initialized `SOCKADDR_INET` built by `sockaddr_inet`. `row` and `source` are
    // valid out-params that `GetBestRoute2` fills on success.
    unsafe { GetBestRoute2(None, 0, None, &destination, 0, &mut row, &mut source) }.ok()?;
    // SAFETY: `GetBestRoute2` succeeded, so each `si_family` names the initialized variant.
    let (source_address, destination) = unsafe { (ip(&source), ip(&row.DestinationPrefix.Prefix)) };
    Ok(BestRoute {
        source_address,
        destination,
        prefix_length: row.DestinationPrefix.PrefixLength,
        interface_index: row.InterfaceIndex,
        interface_alias: interface_alias(row.InterfaceLuid),
        route_metric: row.Metric,
        protocol: row.Protocol.0,
    })
}

fn interface_alias(luid: NET_LUID_LH) -> Option<String> {
    let mut buffer = [0u16; u32_into_usize(IF_MAX_STRING_SIZE) + 1];
    // SAFETY: `luid` is a valid in-param and `buffer` is a writable out-param whose length is passed with it.
    if unsafe { ConvertInterfaceLuidToAlias(&luid, &mut buffer) }.is_err() {
        return None;
    }
    let alias = PWSTR::from_raw(buffer.as_mut_ptr());
    // SAFETY: `ConvertInterfaceLuidToAlias` guarantees that `buffer` (PWSTR's pointer) is null terminated.
    Some(String::from_utf16_lossy(unsafe { alias.as_wide() }))
}

fn sockaddr_inet(ip: IpAddr) -> SOCKADDR_INET {
    match ip {
        IpAddr::V4(ip) => SOCKADDR_INET {
            Ipv4: SOCKADDR_IN {
                sin_family: AF_INET,
                sin_addr: IN_ADDR { S_un: IN_ADDR_0 { S_addr: ip.to_bits().to_be() } },
                ..Default::default()
            },
        },
        IpAddr::V6(ip) => SOCKADDR_INET {
            Ipv6: SOCKADDR_IN6 {
                sin6_family: AF_INET6,
                sin6_addr: IN6_ADDR { u: IN6_ADDR_0 { Byte: ip.octets() } },
                ..Default::default()
            },
        },
    }
}

/// SAFETY: `addr` must have been written by IP Helper (or zeroed), so `si_family` names the initialized variant.
unsafe fn ip(addr: &SOCKADDR_INET) -> Option<IpAddr> {
    // SAFETY: Guaranteed by the caller.
    unsafe {
        if addr.si_family == AF_INET {
            Some(IpAddr::V4(Ipv4Addr::from_bits(u32::from_be(addr.Ipv4.sin_addr.S_un.S_addr))))
        } else if addr.si_family == AF_INET6 {
            Some(IpAddr::V6(Ipv6Addr::from(addr.Ipv6.sin6_addr.u.Byte)))
        } else {
            None
        }
    }
}

//! Enumerates network adapters via `GetAdaptersAddresses`.

use crate::int_helper::u32_into_usize;
use serde::Serialize;
use std::mem::MaybeUninit;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use windows::Win32::Foundation::{ERROR_BUFFER_OVERFLOW, ERROR_NO_DATA, WIN32_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{
    GAA_FLAG_INCLUDE_GATEWAYS, GAA_FLAG_SKIP_ANYCAST, GAA_FLAG_SKIP_MULTICAST, GetAdaptersAddresses, GetIfEntry2, IP_ADAPTER_ADDRESSES_LH,
    MIB_IF_ROW2,
};
use windows::Win32::NetworkManagement::Ndis::NET_LUID_LH;
use windows::Win32::Networking::WinSock::{AF_INET, AF_INET6, AF_UNSPEC, SOCKADDR_IN, SOCKADDR_IN6, SOCKET_ADDRESS};
use windows::core::PWSTR;

/// The subset of `IP_ADAPTER_ADDRESSES_LH` that adapter selection and debug bundles use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct NetworkAdapter {
    /// IPv4 interface index, 0 when IPv4 is not bound to the adapter.
    pub index: u32,
    pub luid: u64,
    pub friendly_name: String,
    pub description: String,
    /// `IF_TYPE_*`: https://microsoft.github.io/windows-docs-rs/doc/windows/Win32/NetworkManagement/IpHelper
    pub if_type: u32,
    /// `IF_OPER_STATUS`: https://learn.microsoft.com/windows/win32/api/ifdef/ne-ifdef-if_oper_status
    pub oper_status: i32,
    /// `HardwareInterface` from `GetIfEntry2`
    pub hardware: Option<bool>,
    pub mtu: u32,
    pub ipv4_metric: u32,
    /// First IPv4 unicast address
    pub ipv4_address: Option<Ipv4Addr>,
    pub gateways: Vec<IpAddr>,
}

pub fn list_network_adapters() -> windows::core::Result<Vec<NetworkAdapter>> {
    let Some(buffer) = GaaBuffer::new()? else {
        return Ok(Vec::new());
    };
    Ok(read_adapters(buffer.first()))
}

struct GaaBuffer {
    buffer: Box<[MaybeUninit<IP_ADAPTER_ADDRESSES_LH>]>,
}

impl GaaBuffer {
    fn new() -> windows::core::Result<Option<Self>> {
        let flags = GAA_FLAG_INCLUDE_GATEWAYS | GAA_FLAG_SKIP_ANYCAST | GAA_FLAG_SKIP_MULTICAST;
        // Recommended initial size, see
        // https://learn.microsoft.com/windows/win32/api/iphlpapi/nf-iphlpapi-getadaptersaddresses#remarks
        let mut buf_len = 15_000u32;
        loop {
            let capacity = u32_into_usize(buf_len).div_ceil(size_of::<IP_ADAPTER_ADDRESSES_LH>());
            let mut buffer = Box::<[IP_ADAPTER_ADDRESSES_LH]>::new_uninit_slice(capacity);
            // SAFETY: `buffer` spans at least `buf_len` bytes. `GetAdaptersAddresses` writes at most `buf_len` bytes
            // and otherwise reports `ERROR_BUFFER_OVERFLOW` with the required size in `buf_len`.
            let ret = unsafe { GetAdaptersAddresses(u32::from(AF_UNSPEC.0), flags, None, Some(buffer.as_mut_ptr().cast()), &mut buf_len) };
            let ret = WIN32_ERROR(ret);
            if ret == ERROR_BUFFER_OVERFLOW {
                continue;
            }
            if ret == ERROR_NO_DATA {
                return Ok(None);
            }
            ret.ok()?;
            return Ok(Some(Self { buffer }));
        }
    }

    fn first(&self) -> *const IP_ADAPTER_ADDRESSES_LH {
        self.buffer.as_ptr().cast()
    }
}

fn read_adapters(mut current: *const IP_ADAPTER_ADDRESSES_LH) -> Vec<NetworkAdapter> {
    let mut adapters = Vec::new();
    while !current.is_null() {
        // SAFETY: `current` is a non-null node of the list `GetAdaptersAddresses`
        // wrote into a buffer that outlives this call.
        let adapter = unsafe { &*current };
        adapters.push(read_adapter(adapter));
        current = adapter.Next;
    }
    adapters
}

/// `adapter` is a node written by `GetAdaptersAddresses`, which the unsafe reads below rely on.
fn read_adapter(adapter: &IP_ADAPTER_ADDRESSES_LH) -> NetworkAdapter {
    NetworkAdapter {
        // SAFETY: Both union members are plain integers, so every bit pattern is a valid read.
        index: unsafe { adapter.Anonymous1.Anonymous.IfIndex },
        // SAFETY: Same, the LUID union is a `u64` and a bitfield struct of the same size.
        luid: unsafe { adapter.Luid.Value },
        // SAFETY: `GetAdaptersAddresses` writes a valid `PWSTR`.
        friendly_name: unsafe { pwstr_to_string(adapter.FriendlyName) },
        // SAFETY: Same as `friendly_name`.
        description: unsafe { pwstr_to_string(adapter.Description) },
        if_type: adapter.IfType,
        oper_status: adapter.OperStatus.0,
        hardware: is_hardware_interface(adapter.Luid),
        mtu: adapter.Mtu,
        ipv4_metric: adapter.Ipv4Metric,
        // SAFETY: `FirstUnicastAddress` heads a list of unicast nodes in the same buffer as `adapter`.
        ipv4_address: unsafe { list_nodes(adapter.FirstUnicastAddress, |node| node.Next) }
            .into_iter()
            .find_map(|node| {
                // SAFETY: `Address` of every node was written by `GetAdaptersAddresses`.
                match unsafe { socket_address_to_ip(&node.Address) } {
                    Some(IpAddr::V4(ip)) => Some(ip),
                    _ => None,
                }
            }),
        // SAFETY: `FirstGatewayAddress` heads a list of gateway nodes in the same buffer as `adapter`.
        gateways: unsafe { list_nodes(adapter.FirstGatewayAddress, |node| node.Next) }
            .into_iter()
            .filter_map(|node| {
                // SAFETY: `Address` of every node was written by `GetAdaptersAddresses`.
                unsafe { socket_address_to_ip(&node.Address) }
            })
            .collect(),
    }
}

/// Collects the nodes of a singly linked list starting at `first`.
///
/// SAFETY: `first` must be null or point to a valid list whose nodes stay alive for `'a`.
unsafe fn list_nodes<'a, T>(first: *mut T, next: impl Fn(&T) -> *mut T) -> Vec<&'a T> {
    let mut nodes = Vec::new();
    let mut current = first;
    while !current.is_null() {
        // SAFETY: Guaranteed by the caller.
        let node = unsafe { &*current };
        nodes.push(node);
        current = next(node);
    }
    nodes
}

/// SAFETY: `address` must come from a `GetAdaptersAddresses` list, which guarantees
/// `lpSockaddr` points to `iSockaddrLength` readable bytes.
unsafe fn socket_address_to_ip(address: &SOCKET_ADDRESS) -> Option<IpAddr> {
    let sockaddr = address.lpSockaddr;
    if sockaddr.is_null() {
        return None;
    }
    let len = usize::try_from(address.iSockaddrLength).ok()?;
    // SAFETY: Non-null and readable per the caller. The family check selects the matching layout and the length
    // check ensures that layout is fully in bounds.
    unsafe {
        let family = (*sockaddr).sa_family;
        if family == AF_INET && len >= size_of::<SOCKADDR_IN>() {
            let sockaddr_in = &*sockaddr.cast::<SOCKADDR_IN>();
            Some(IpAddr::V4(Ipv4Addr::from_bits(u32::from_be(sockaddr_in.sin_addr.S_un.S_addr))))
        } else if family == AF_INET6 && len >= size_of::<SOCKADDR_IN6>() {
            let sockaddr_in6 = &*sockaddr.cast::<SOCKADDR_IN6>();
            Some(IpAddr::V6(Ipv6Addr::from(sockaddr_in6.sin6_addr.u.Byte)))
        } else {
            None
        }
    }
}

fn is_hardware_interface(luid: NET_LUID_LH) -> Option<bool> {
    let mut row = MIB_IF_ROW2 { InterfaceLuid: luid, ..Default::default() };
    // SAFETY: `row` is a valid out-param keyed by `InterfaceLuid`.
    if unsafe { GetIfEntry2(&mut row) }.is_err() {
        return None;
    }
    // https://learn.microsoft.com/windows/win32/api/netioapi/ns-netioapi-mib_if_row2
    Some(row.InterfaceAndOperStatusFlags._bitfield & 1 /* HardwareInterface */ != 0)
}

/// SAFETY: `s` must be null or a valid NUL-terminated wide string.
unsafe fn pwstr_to_string(s: PWSTR) -> String {
    if s.is_null() {
        return String::new();
    }
    // SAFETY: Guaranteed by the caller.
    String::from_utf16_lossy(unsafe { s.as_wide() })
}

/// Runs against the real adapter list of the machine executing the tests.
#[test]
fn lists_adapters() {
    let adapters = list_network_adapters().unwrap();
    assert!(adapters.iter().any(|adapter| adapter.if_type == 24 /* IF_TYPE_SOFTWARE_LOOPBACK */));
    println!("{}", serde_json::to_string_pretty(&adapters).unwrap());
}

use crate::positive_u31::PositiveU31;
use crate::quicwg::{DEFAULT_UDP_PAYLOAD_SIZE, IPV4_UDP_OVERHEAD};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use std::io;
use std::net::{Ipv4Addr, SocketAddrV4};
#[cfg(not(target_os = "windows"))]
use std::os::fd::AsRawFd;
#[cfg(not(target_os = "windows"))]
use std::ptr::addr_of_mut;
#[cfg(not(target_os = "windows"))]
use std::{mem, ptr};

#[derive(PartialEq, Eq, Clone, Debug, Deserialize, Serialize)]
pub struct NetworkInterface {
    pub name: String,
    pub index: PositiveU31,
    #[cfg(target_os = "windows")]
    pub ip: std::net::IpAddr,
    #[cfg(target_os = "windows")]
    pub mtu: i32,
}

pub fn new_udp(network_interface: Option<&NetworkInterface>) -> io::Result<std::net::UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    #[cfg(not(any(target_os = "android", target_os = "windows")))]
    if let Some(network_interface) = network_interface {
        socket.bind_device_by_index_v4(Some(network_interface.index.into()))?;
    }
    #[allow(unused_mut)]
    let mut bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0).into();
    #[cfg(target_os = "windows")]
    if let Some(interface) = network_interface {
        bind_addr = std::net::SocketAddr::new(interface.ip, 0).into();
    }
    #[cfg(target_os = "android")]
    {
        _ = network_interface;
    }
    socket.bind(&bind_addr)?;
    Ok(socket.into())
}

#[cfg(not(any(target_os = "ios", target_os = "macos", target_os = "windows")))]
const SIOCGIFMTU: libc::Ioctl = libc::SIOCGIFMTU as libc::Ioctl;

#[cfg(any(target_os = "ios", target_os = "macos"))]
const SIOCGIFMTU: libc::c_ulong = 3223349555; // From sys/sockio.h.

#[cfg(not(target_os = "windows"))]
pub fn interface_mtu(interface: &NetworkInterface) -> anyhow::Result<i32> {
    let name = &interface.name;
    let socket = Socket::new_raw(Domain::IPV4, Type::DGRAM, None)?;

    let mut name_buf: [u8; libc::IFNAMSIZ] = [0; _];
    // Note: It isn't clear if the name needs to be null terminated if it is the maximum length but we just assume so.
    anyhow::ensure!(name_buf.len() > name.len(), "Interface name too long.");
    name_buf[..name.len()].copy_from_slice(name.as_bytes());
    let name_buf: [libc::c_char; libc::IFNAMSIZ] = unsafe { mem::transmute(name_buf) };

    unsafe {
        let mut ifreq = mem::MaybeUninit::<libc::ifreq>::uninit();
        ptr::write(addr_of_mut!((*ifreq.as_mut_ptr()).ifr_name), name_buf);

        let r = libc::ioctl(socket.as_raw_fd(), SIOCGIFMTU, ifreq.as_mut_ptr());
        if r < 0 {
            Err(io::Error::last_os_error().into())
        } else {
            Ok(ifreq.assume_init().ifr_ifru.ifru_mtu)
        }
    }
}

#[cfg(target_os = "windows")]
pub fn interface_mtu(interface: &NetworkInterface) -> anyhow::Result<i32> {
    Ok(interface.mtu)
}

pub fn new_quic(udp: std::net::UdpSocket, mtu: Option<u16>, force_small_mtu: bool) -> anyhow::Result<quinn::Endpoint> {
    let runtime = quinn::default_runtime().context("no quinn runtime found")?;
    let mut endpoint_config = quinn::EndpointConfig::default();
    if mtu.is_some_and(|mtu| mtu < DEFAULT_UDP_PAYLOAD_SIZE + IPV4_UDP_OVERHEAD) || force_small_mtu {
        match force_small_mtu {
            true => tracing::info!(
                message_id = "kq0AuTsT",
                "forcing relay to use small UDP payload due to small MTU experimental flag being set"
            ),
            false => tracing::info!(
                message_id = "TF51QUHb",
                mtu,
                "forcing relay to use small UDP payload due to low network MTU"
            ),
        }
        // TODO: Remove once relays does MTU discovery https://linear.app/soveng/issue/OBS-3201/replace-client-side-max-udp-payload-size-constraint-with-relay-side
        endpoint_config
            // A less conservative udp payload size could be calculated as `mtu - IPV4_UDP_OVERHEAD`, but:
            // - this is an uncommon case (for networks with very low MTU)
            // - packet size distribution tends to be bimodal, the exact fragmentation threshold doesn't matter much
            // - technically QUIC and IP overhead aren't fixed
            // - this will be removed once the relay supports MTU discovery
            // - 1200 is the hard lower limit for QUIC and easily fits WG fragments and has the best compatibility with low-MTU network environments
            .max_udp_payload_size(1200)
            .context("invalid max_udp_payload_size")?;
    }
    let endpoint = quinn::Endpoint::new(endpoint_config, None, udp, runtime)?;
    Ok(endpoint)
}

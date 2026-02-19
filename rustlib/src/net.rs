use crate::positive_u31::PositiveU31;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use socket2::{Domain, Protocol, Socket, Type};
use std::net::{Ipv4Addr, SocketAddrV4};
use std::os::fd::AsRawFd;
use std::ptr::addr_of_mut;
use std::{io, mem, ptr};

#[derive(PartialEq, Eq, Clone, Debug, Deserialize, Serialize)]
pub struct NetworkInterface {
    pub name: String,
    pub index: PositiveU31,
    #[cfg(target_os = "windows")]
    pub ip: std::net::IpAddr,
}

pub fn new_udp(network_interface: Option<&NetworkInterface>) -> io::Result<std::net::UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    #[cfg(not(target_os = "windows"))]
    if let Some(network_interface) = network_interface {
        socket.bind_device_by_index_v4(Some(network_interface.index.into()))?;
    }
    #[allow(unused_mut)]
    let mut bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0).into();
    #[cfg(target_os = "windows")]
    if let Some(interface) = network_interface {
        bind_addr = std::net::SocketAddr::new(interface.ip, 0).into();
    }
    socket.bind(&bind_addr)?;
    Ok(socket.into())
}

#[cfg(not(any(target_os = "ios", target_os = "macos")))]
const SIOCGIFMTU: libc::Ioctl = libc::SIOCGIFMTU as libc::Ioctl;

#[cfg(any(target_os = "ios", target_os = "macos"))]
const SIOCGIFMTU: libc::c_ulong = 3223349555; // From sys/sockio.h.

pub fn interface_mtu(name: &str) -> anyhow::Result<i32> {
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

pub fn new_quic(udp: std::net::UdpSocket) -> anyhow::Result<quinn::Endpoint> {
    let runtime = quinn::default_runtime().context("no quinn runtime found")?;
    let endpoint = quinn::Endpoint::new(Default::default(), None, udp, runtime)?;
    Ok(endpoint)
}

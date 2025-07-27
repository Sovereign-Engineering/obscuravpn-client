use anyhow::Context;
use socket2::{Domain, Protocol, Socket, Type};
use std::io;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::num::NonZeroU32;

pub fn new_udp(fw_mark: Option<u32>, network_interface_index: Option<NonZeroU32>) -> io::Result<std::net::UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    if let Some(network_interface_index) = network_interface_index {
        socket.bind_device_by_index_v4(Some(network_interface_index))?;
    }
    let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0).into();
    socket.bind(&bind_addr)?;
    if let Some(fw_mark) = fw_mark {
        _ = fw_mark;
        #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
        socket.set_mark(fw_mark)?;
        #[cfg(not(any(target_os = "android", target_os = "fuchsia", target_os = "linux")))]
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "fw_mark only supported on android, linux and fuchsia",
        ));
    }
    Ok(socket.into())
}

pub fn new_quic(udp: std::net::UdpSocket) -> anyhow::Result<quinn::Endpoint> {
    let runtime = quinn::default_runtime().context("no quinn runtime found")?;
    let endpoint = quinn::Endpoint::new(Default::default(), None, udp, runtime)?;
    Ok(endpoint)
}

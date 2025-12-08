use crate::positive_u31::PositiveU31;
use anyhow::Context;
use socket2::{Domain, Protocol, Socket, Type};
use std::io;
use std::net::{Ipv4Addr, SocketAddrV4};

#[derive(PartialEq, Eq, Clone, Debug)]
pub struct NetworkInterface {
    pub name: String,
    pub index: PositiveU31,
}

pub fn new_udp(network_interface: Option<&NetworkInterface>) -> io::Result<std::net::UdpSocket> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    if let Some(network_interface) = network_interface {
        socket.bind_device_by_index_v4(Some(network_interface.index.into()))?;
    }
    let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0).into();
    socket.bind(&bind_addr)?;
    Ok(socket.into())
}

pub fn new_quic(udp: std::net::UdpSocket) -> anyhow::Result<quinn::Endpoint> {
    let runtime = quinn::default_runtime().context("no quinn runtime found")?;
    let endpoint = quinn::Endpoint::new(Default::default(), None, udp, runtime)?;
    Ok(endpoint)
}

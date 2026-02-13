use crate::service::os::PutIncomingPacketFn;
use std::sync::Weak;

pub struct Tun {}

pub struct TunWriter {}

impl Tun {
    pub fn new() -> Self {
        Tun {}
    }
    pub fn writer(&self) -> TunWriter {
        TunWriter {}
    }
}

impl PutIncomingPacketFn for TunWriter {
    fn call(&mut self, packet: &[u8]) {
        tracing::info!("manager provided incoming packet: {}", packet.len());
    }
}

impl TunWriter {
    pub const fn invalid() -> Self {
        Self {}
    }
}

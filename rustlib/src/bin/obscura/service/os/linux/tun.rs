use crate::service::os::PutIncomingPacketFn;
use crate::service::os::packet_buffer::PacketBuffer;
use ipnetwork::Ipv6Network;
use obscuravpn_client::net::NetworkInterface;
use obscuravpn_client::network_config::TunnelNetworkConfig;
use obscuravpn_client::positive_u31::PositiveU31;
use std::io::ErrorKind::{AlreadyExists, WouldBlock};
use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

const TUN_MIN_LOG_SILENCE: Duration = Duration::from_secs(5);
const TUN_NAME: &str = "obscuravpn";

pub struct Tun {
    dev: Arc<tun_rs::AsyncDevice>,
    interface_index: PositiveU31,
    last_error_log_at: Mutex<Option<Instant>>,
}

pub struct TunWriter {
    dev: Weak<tun_rs::AsyncDevice>,
    last_error_log_at: Option<Instant>,
}

impl Tun {
    pub async fn create() -> anyhow::Result<Self> {
        let network_config = TunnelNetworkConfig::dummy();
        let dev = Arc::new(
            tun_rs::DeviceBuilder::new()
                // NetworkManager classifies new TUN devices without assigned IPs as `NM_DEVICE_STATE_UNMANAGED` instead of just externally connected and refuses all device configuration interactions. As initial state this is harmless in tested versions, but avoiding the state is simpler and may be safer.
                .ipv4(network_config.ipv4, 32u8, None)
                .ipv6(network_config.ipv6.network(), network_config.ipv6.prefix())
                .mtu(network_config.mtu)
                .name(TUN_NAME.to_string())
                .build_async()?,
        );
        let interface_index = dev.if_index()?.try_into()?;
        Ok(Self { dev, interface_index, last_error_log_at: Mutex::new(None) })
    }

    pub fn interface(&self) -> NetworkInterface {
        NetworkInterface { name: TUN_NAME.to_string(), index: self.interface_index }
    }

    pub fn writer(&self) -> TunWriter {
        TunWriter { dev: Arc::downgrade(&self.dev), last_error_log_at: None }
    }

    pub async fn receive(&self, packet_buffer: &mut PacketBuffer) {
        if let Err(error) = self.dev.readable().await {
            let mut last_error_log_at = self.last_error_log_at.lock().unwrap();
            rate_limited_error_log(&mut last_error_log_at, "YRah33os", "failed to wait for packet on tun device", error)
        }
        while let Some(buffer) = packet_buffer.buffer() {
            match self.dev.try_recv(buffer) {
                Ok(n) => match u16::try_from(n) {
                    Ok(n) => packet_buffer.commit(n),
                    Err(_) => {
                        let mut last_error_log_at = self.last_error_log_at.lock().unwrap();
                        rate_limited_error_log(
                            &mut last_error_log_at,
                            "A1s4jdil",
                            "ignoring oversized packet from tun device",
                            std::io::Error::other("oversized packet"),
                        )
                    }
                },
                Err(error) if error.kind() == WouldBlock => return,
                Err(error) => {
                    let mut last_error_log_at = self.last_error_log_at.lock().unwrap();
                    rate_limited_error_log(&mut last_error_log_at, "uGIH5zSb", "failed to receive from tun device", error)
                }
            }
        }
    }

    pub fn set_config(&mut self, mtu: u16, ipv4: Ipv4Addr, ipv6: Ipv6Network) -> Result<(), ()> {
        let mut result = Ok(());

        // Add new IPs before removing the current ones. This prevents having no addresses on the device temporarily, which may trigger automatic network manager device state changes with unintended side effects on DNS and routes.

        if let Err(error) = self.dev.set_mtu(mtu) {
            tracing::error!(message_id = "qPppmh83", ?error, "failed to set tun mtu");
            result = Err(());
        }
        if let Err(error) = self.dev.add_address_v4(ipv4, 32u8)
            && error.kind() != AlreadyExists
        {
            tracing::error!(message_id = "cY11X3I6", ?error, address = ?ipv4, "failed to add IPv4 tun address");
            result = Err(());
        }
        if let Err(error) = self.dev.add_address_v6(ipv6.network(), ipv6.prefix())
            && error.kind() != AlreadyExists
        {
            tracing::error!(message_id = "wHod6P2h", ?error, address = ?ipv6, "failed to add IPv6 tun address");
            result = Err(());
        }

        match self.dev.addresses() {
            Ok(addresses) => {
                for address in addresses {
                    let keep = match address {
                        IpAddr::V4(address) => address == ipv4,
                        IpAddr::V6(address) => ipv6.contains(address),
                    };
                    if keep {
                        continue;
                    }
                    if let Err(error) = self.dev.remove_address(address) {
                        tracing::error!(message_id = "qPppmh83", ?error, ?address, "failed to remove tun address");
                        result = Err(());
                    }
                }
            }
            Err(error) => {
                tracing::error!(message_id = "1SDywPMm", ?error, "failed to retrieve tun addresses");
                result = Err(());
            }
        }
        result
    }
}

impl TunWriter {
    pub const fn invalid() -> Self {
        Self { dev: Weak::new(), last_error_log_at: None }
    }
}

impl PutIncomingPacketFn for TunWriter {
    fn call(&mut self, packet: &[u8]) {
        let Some(dev) = self.dev.upgrade() else {
            rate_limited_error_log(
                &mut self.last_error_log_at,
                "blWRxJIQ",
                "send on dropped or invalid tun device",
                std::io::Error::other("no device"),
            );
            return;
        };
        if let Err(error) = dev.try_send(packet)
            && error.kind() != WouldBlock
        {
            rate_limited_error_log(&mut self.last_error_log_at, "4nG6rvr3", "failed to send packet on tun device", error);
        }
    }
}

fn rate_limited_error_log(last: &mut Option<Instant>, message_id: &'static str, message: &'static str, error: std::io::Error) {
    let now = Instant::now();
    if last.is_some_and(|last| last + TUN_MIN_LOG_SILENCE > now) {
        return;
    }
    *last = Some(now);
    tracing::error!(message_id, ?error, message);
}

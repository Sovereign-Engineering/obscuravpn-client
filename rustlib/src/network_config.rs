use ipnetwork::Ipv6Network;
use obscuravpn_api::types::ObfuscatedTunnelConfig;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use strum::EnumIs;
use thiserror::Error;

// Keep synchronized with ../apple/system-network-extension/RustFfi.swift
// Avoid adding information with high-frequency of change to this type, to prevent triggering frequent changes OS network configuration, which can't be deduplicated by checking for changes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TunnelNetworkConfig {
    pub dns: Vec<IpAddr>,
    pub ipv4: Ipv4Addr,
    pub ipv6: Ipv6Network,
    pub mtu: u16,
}

impl TunnelNetworkConfig {
    pub fn new(tunnel_config: &ObfuscatedTunnelConfig, mtu: u16) -> Result<Self, NetworkConfigError> {
        let dns = tunnel_config.dns.clone();
        if dns.is_empty() {
            return Err(NetworkConfigError::NoDns);
        }

        let Some(ipv4) = tunnel_config.client_ips_v4.first().map(|net| net.ip()) else {
            return Err(NetworkConfigError::NoIpv4Ip);
        };

        let Some(ipv6) = tunnel_config.client_ips_v6.first().cloned() else {
            return Err(NetworkConfigError::NoIpv6Ip);
        };

        Ok(Self { dns, ipv4, ipv6, mtu })
    }

    /// Dummy network config. May be used if valid values are needed by an API before the real values are known. The values are picked from ranges we expect for our tunnels.
    pub fn dummy() -> Self {
        Self {
            dns: vec![IpAddr::V4(Ipv4Addr::new(10, 64, 0, 99))],
            ipv4: Ipv4Addr::new(10, 75, 76, 77),
            ipv6: Ipv6Network::new(Ipv6Addr::new(0xfc00, 0xbbbb, 0xbbbb, 0xbb01, 0, 0, 0xc, 0x4c4d), 128).unwrap(),
            mtu: 1280,
        }
    }

    pub fn apply_dns_content_block(&mut self, exit_provider_name: &str, dns_content_block: DnsContentBlock) {
        if let Some(dns) = dns_content_block.mullvad_dns_ip()
            && exit_provider_name == "Mullvad VPN"
        {
            self.dns = vec![dns.into()];
        }
    }
}

#[derive(Clone, Debug, Error)]
pub enum NetworkConfigError {
    #[error("no ipv4 ip")]
    NoIpv4Ip,
    #[error("no ipv6 ip")]
    NoIpv6Ip,
    #[error("no dns")]
    NoDns,
}

#[derive(Clone, Copy, Debug, Default, EnumIs, PartialEq, Eq, Serialize, Deserialize)]
pub enum DnsConfig {
    #[default]
    Default,
    System,
}

#[derive(Default, Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DnsContentBlock {
    ad: bool,
    tracker: bool,
    malware: bool,
    adult: bool,
    gambling: bool,
    social_media: bool,
}

impl DnsContentBlock {
    pub fn mullvad_dns_ip(self) -> Option<Ipv4Addr> {
        let bitset = u8::from(self.ad)
            | (u8::from(self.tracker) << 1)
            | (u8::from(self.malware) << 2)
            | (u8::from(self.adult) << 3)
            | (u8::from(self.gambling) << 4)
            | (u8::from(self.social_media) << 5);
        (bitset != 0).then_some(Ipv4Addr::new(100, 64, 0, bitset))
    }
}

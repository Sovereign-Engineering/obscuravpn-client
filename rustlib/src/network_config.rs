use ipnetwork::Ipv6Network;
use obscuravpn_api::types::ObfuscatedTunnelConfig;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr};
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

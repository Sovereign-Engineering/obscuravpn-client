use crate::{
    config::ConfigDebug,
    debug_archive::{dns::DnsTask, task::DebugTask},
    net::NetworkInterface,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DebugInfo {
    pub config: ConfigDebug,
    pub dns_apple: DebugTask<DnsTask>,
    pub dns_google: DebugTask<DnsTask>,
    pub dns_obscura: DebugTask<DnsTask>,
    pub network_interface: Option<NetworkInterface>,
    pub network_interface_mtu: Option<i32>,
}

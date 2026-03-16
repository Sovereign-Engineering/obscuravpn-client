use crate::{
    config::ConfigDebug,
    debug_archive::{dns::DnsTask, http::HttpTask, task::DebugTask},
    net::NetworkInterface,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DebugInfo {
    pub config: ConfigDebug,
    pub dns_apple: DebugTask<DnsTask>,
    pub dns_google: DebugTask<DnsTask>,
    pub dns_obscura: DebugTask<DnsTask>,
    pub http_apple: DebugTask<HttpTask>,
    pub http_google: DebugTask<HttpTask>,
    pub http_nosni: DebugTask<HttpTask>,
    pub http_obscura: DebugTask<HttpTask>,
    pub network_interface: Option<NetworkInterface>,
    pub network_interface_mtu: Option<i32>,
}

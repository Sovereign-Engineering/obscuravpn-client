use crate::{
    config::ConfigDebug,
    debug_bundle::{dns::DebugTaskDns, http::DebugTaskHttp, task::DebugTask},
    net::NetworkInterface,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct DebugInfo {
    pub config: ConfigDebug,
    pub dns_apple: DebugTask<DebugTaskDns>,
    pub dns_google: DebugTask<DebugTaskDns>,
    pub dns_obscura: DebugTask<DebugTaskDns>,
    pub http_apple: DebugTask<DebugTaskHttp>,
    pub http_google: DebugTask<DebugTaskHttp>,
    pub http_nosni: DebugTask<DebugTaskHttp>,
    pub http_obscura: DebugTask<DebugTaskHttp>,
    pub http_obscura_apple: DebugTask<DebugTaskHttp>,
    pub http_obscura_google: DebugTask<DebugTaskHttp>,
    pub network_interface: Option<NetworkInterface>,
    pub network_interface_mtu: Option<i32>,
}

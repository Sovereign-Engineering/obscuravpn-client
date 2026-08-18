use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use tokio::net::lookup_host;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DebugTaskDns {
    pub host: String,
    pub addrs: Vec<IpAddr>,
}

impl DebugTaskDns {
    pub async fn run(host: &'static str) -> Result<Self, Box<dyn std::error::Error>> {
        let addrs = lookup_host((host, 443)).await?.map(|socket_addr| socket_addr.ip()).collect();
        Ok(Self { host: host.to_owned(), addrs })
    }
}

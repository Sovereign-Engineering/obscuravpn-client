use crate::debug_archive::task::DebugTask;
use crate::debug_archive::task::run_debug_task;
use serde::Deserialize;
use serde::Serialize;
use std::net::IpAddr;
use tokio::net::lookup_host;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DnsTask {
    pub addrs: Vec<IpAddr>,
}

pub async fn debug_dns(host_port: &'static str) -> DebugTask<DnsTask> {
    run_debug_task(async { Ok(DnsTask { addrs: lookup_host(host_port).await?.map(|socket_addr| socket_addr.ip()).collect() }) }).await
}

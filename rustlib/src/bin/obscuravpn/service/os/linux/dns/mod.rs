use obscuravpn_client::net::NetworkInterface;
use std::net::IpAddr;

mod network_manager;
mod resolved;

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum DnsManager {
    Resolved,
    NetworkManager,
}

pub async fn detect_dns_manager() -> Option<DnsManager> {
    let network_manager = network_manager::detect().await;
    let resolved = resolved::detect().await;
    let pick = if resolved {
        Some(DnsManager::Resolved)
    } else if network_manager {
        Some(DnsManager::NetworkManager)
    } else {
        None
    };
    tracing::info!(message_id = "PsaY3ZPO", network_manager, resolved, ?pick, "DNS manager detection");
    pick
}

pub async fn set_dns(tun: &NetworkInterface, dns: &[IpAddr]) -> Result<(), ()> {
    match detect_dns_manager().await.ok_or(())? {
        DnsManager::Resolved => resolved::set_dns(tun, dns).await,
        DnsManager::NetworkManager => network_manager::set_dns(tun, dns).await,
    }
}

pub async fn reset_dns(tun: &NetworkInterface) -> Result<(), ()> {
    match detect_dns_manager().await.ok_or(())? {
        DnsManager::Resolved => resolved::reset_dns(tun).await,
        DnsManager::NetworkManager => network_manager::reset_dns(tun).await,
    }
}

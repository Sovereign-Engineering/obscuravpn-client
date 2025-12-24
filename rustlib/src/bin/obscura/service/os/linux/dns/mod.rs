use crate::DnsManagerArg;
use crate::service::os::linux::network_manager;

pub mod resolved;

#[derive(Debug, Copy, Clone, Eq, PartialEq, strum::EnumIs)]
pub enum DnsManager {
    Disabled,
    Resolved,
    NetworkManager,
}

pub async fn choose_dns_manager(dns_manager_arg: DnsManagerArg) -> Result<DnsManager, ()> {
    let network_manager = network_manager::detect().await;
    let resolved = resolved::detect().await;

    let choice = match dns_manager_arg {
        DnsManagerArg::Disabled => Ok(DnsManager::Disabled),
        DnsManagerArg::Auto => {
            if resolved {
                Ok(DnsManager::Resolved)
            } else if network_manager {
                Ok(DnsManager::NetworkManager)
            } else {
                tracing::error!(message_id = "ltV4egoX", "no supported DNS manager detected");
                Err(())
            }
        }
        DnsManagerArg::NetworkManager if network_manager => Ok(DnsManager::NetworkManager),
        DnsManagerArg::Resolved if resolved => Ok(DnsManager::Resolved),
        dns_manager_arg => {
            tracing::error!(message_id = "bJO46yTy", ?dns_manager_arg, "requested DNS manager not detected");
            Err(())
        }
    };
    tracing::info!(
        message_id = "PsaY3ZPO",
        ?dns_manager_arg,
        network_manager,
        resolved,
        ?choice,
        "DNS manager detection"
    );
    choice
}

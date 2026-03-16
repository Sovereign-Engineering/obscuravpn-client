use crate::client_state::WeakClientStateHandle;
use obscuravpn_api::reexports::reqwest::dns::{Addrs, Name, Resolve, Resolving};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

pub struct DnsResolver {
    client_state: WeakClientStateHandle,
}

impl DnsResolver {
    pub fn new(client_state: WeakClientStateHandle) -> Arc<Self> {
        Arc::new(Self { client_state })
    }
}

impl Resolve for DnsResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let resolving = tokio::spawn(resolve_and_cache(self.client_state.clone(), name.as_str().to_string()));

        let cached = match self.client_state.upgrade() {
            None => {
                tracing::warn!(message_id = "F0ZR7dTm", "can't read from DNS cache of dropped client state");
                Vec::new()
            }
            Some(client_state) => client_state.borrow().config().dns_cache.get(name.as_str()),
        };
        if !cached.is_empty() {
            let addrs: Addrs = Box::new(cached.into_iter());
            return Box::pin(std::future::ready(Ok(addrs)));
        }

        tracing::warn!(message_id = "ooPh8ahc", name = name.as_str(), "DNS cache miss, wait for resolver");
        Box::pin(async move {
            let addrs = resolving.await.expect("DNS resolution task panicked")?;
            let addrs: Addrs = Box::new(addrs.into_iter());
            Ok(addrs)
        })
    }
}

async fn resolve_and_cache(client_state: WeakClientStateHandle, name: String) -> Result<Vec<SocketAddr>, Box<dyn std::error::Error + Send + Sync>> {
    const TIMEOUT: Duration = Duration::from_secs(60);

    let name = name.as_str();
    match timeout(TIMEOUT, tokio::net::lookup_host((name, 0u16))).await {
        Ok(Ok(addrs)) => {
            let addrs: Vec<_> = addrs.collect();
            if !addrs.is_empty() {
                tracing::info!(message_id = "ea1Ooquu", name, ?addrs, "DNS resolution succeeded");
                match client_state.upgrade() {
                    None => tracing::warn!(message_id = "2aZU1KWD", "can't write to DNS cache of dropped client state"),
                    Some(client_state) => client_state.update_dns_cache(name, &addrs),
                }
            } else {
                tracing::warn!(message_id = "Uu3ohPh4", name, "DNS resolution returned no addresses");
            }
            Ok(addrs)
        }
        Ok(Err(error)) => {
            tracing::warn!(message_id = "ieC5ahv3", name, ?error, "DNS resolution failed: {error}");
            Err(error.into())
        }
        Err(error) => {
            tracing::warn!(message_id = "RwX9EdwE", name, ?error, "DNS resolution timed out: {error}");
            Err(error.into())
        }
    }
}

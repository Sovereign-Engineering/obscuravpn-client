use crate::network_config::OsNetworkConfig;
use crate::quicwg::QuicWgConnPacketSender;
use bytes::Bytes;
use std::future::pending;
use std::sync::Arc;
use tokio::sync::RwLock;

pub trait Os: Sync + Send + 'static {
    /// Set the network state. Returning `Ok()` implies that the OS will route traffic to the tunnel. May be called repeatedly before the tunnel is functional or after the tunnel started relaying traffic to reflect changing IP Address or DNS configuration. Regardless of errors that may occur, the implementation should set up as much routing/filtering as possible to avoid leaking traffic.
    /// Will not be called concurrently with itself or `unset_os_network_config`.
    // TODO: Consider moving this to its own trait with `&mut` receiver and remove sentence above.
    fn set_os_network_config(&self, network_config: OsNetworkConfig, tunnel: QuicWgConnPacketSender) -> impl Future<Output = Result<(), ()>> + Send;

    /// Reset the network state. Returning `Ok()` implies that the OS will stop routing traffic to the tunnel soon.
    fn unset_os_network_config(&self) -> impl Future<Output = Result<(), ()>> + Send;

    /// Will be called when a packet from the relay is received on the tunnel, which should be emitted on the tunnel device.
    fn packet_for_os(&self, packet: Bytes);
}

/// Revocable [`Os`] access: after [`RevocableOs::revoke`] returns, network config calls block forever and packets are dropped.
pub struct RevocableOs<O: Os> {
    inner: RwLock<Option<Arc<O>>>,
}

impl<O: Os> RevocableOs<O> {
    pub fn new(os_impl: Arc<O>) -> Self {
        Self { inner: RwLock::new(Some(os_impl)) }
    }

    pub async fn revoke(&self) {
        tracing::info!(message_id = "aTn7RJgd", "revoking access to OS network integration");
        if self.inner.write().await.take().is_none() {
            tracing::error!(message_id = "e2vWFJqA", "access to OS network integration was already revoked");
        }
    }
}

impl<O: Os> Os for RevocableOs<O> {
    async fn set_os_network_config(&self, network_config: OsNetworkConfig, tunnel: QuicWgConnPacketSender) -> Result<(), ()> {
        let os_impl_guard = self.inner.read().await;
        let Some(os_impl) = os_impl_guard.as_deref() else {
            drop(os_impl_guard);
            tracing::info!(message_id = "yGpMuTz4", "set_os_network_config called after revocation, blocking forever");
            return pending().await;
        };
        os_impl.set_os_network_config(network_config, tunnel).await
    }

    async fn unset_os_network_config(&self) -> Result<(), ()> {
        let os_impl_guard = self.inner.read().await;
        let Some(os_impl) = os_impl_guard.as_deref() else {
            drop(os_impl_guard);
            tracing::info!(
                message_id = "fQ2nWjbK",
                "unset_os_network_config called after revocation, blocking forever"
            );
            return pending().await;
        };
        os_impl.unset_os_network_config().await
    }

    fn packet_for_os(&self, packet: Bytes) {
        let Ok(os_impl_guard) = self.inner.try_read() else { return };
        let Some(os_impl) = os_impl_guard.as_deref() else { return };
        os_impl.packet_for_os(packet);
    }
}

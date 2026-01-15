use obscuravpn_client::net::NetworkInterface;
use std::net::IpAddr;
use zbus_systemd::zbus;

async fn zbus_connect() -> Result<zbus_systemd::resolve1::ManagerProxy<'static>, ()> {
    let conn = zbus::Connection::system()
        .await
        .map_err(|error| tracing::error!(message_id = "SX4gJ91O", ?error, "failed to create DBUS system connection: {}", error))?;
    zbus_systemd::resolve1::ManagerProxy::new(&conn)
        .await
        .map_err(|error| tracing::error!(message_id = "AucCE8My", ?error, "failed to create resolved zbus proxy: {}", error))
        .map(|proxy| proxy.to_owned())
}

// Returns true if resolved is running and in stub mode
pub async fn detect() -> bool {
    let Ok(proxy) = zbus_connect().await else {
        return false;
    };
    match proxy.resolv_conf_mode().await {
        Ok(mode) => {
            tracing::info!(message_id = "0TsSfY4K", mode, "resolved is running");
            mode == "stub"
        }
        Err(error) => {
            tracing::error!(message_id = "DDMvhHf4", ?error, "failed to query resolved mode: {}", error);
            false
        }
    }
}

pub async fn set_dns(tun: &NetworkInterface, dns: &[IpAddr]) -> Result<(), ()> {
    let dns = dns
        .iter()
        .map(|entry| match entry {
            IpAddr::V4(entry) => (libc::AF_INET, entry.octets().to_vec()),
            IpAddr::V6(entry) => (libc::AF_INET6, entry.octets().to_vec()),
        })
        .collect();
    // Equivalent to `resolvectl dns obscura <DNS IP>`
    let proxy = zbus_connect().await?;
    proxy
        .set_link_dns(tun.index.into(), dns)
        .await
        .map_err(|error| tracing::error!(message_id = "H7vih0nS", ?error, "failed to set tun DNS IPs: {}", error))?;
    // Equivalent to `resolvectl domain obscuravpn ~.`. The `~` (or `true`) below, indicates a routing-only domain (not search domain)
    proxy
        .set_link_domains(tun.index.into(), vec![(".".to_string(), true)])
        .await
        .map_err(|error| tracing::error!(message_id = "92tR6ndT", ?error, "failed to set tun DNS domain: {}", error))?;
    Ok(())
}

pub async fn reset_dns(tun: &NetworkInterface) -> Result<(), ()> {
    zbus_connect()
        .await?
        .revert_link(tun.index.into())
        .await
        .map_err(|error| tracing::error!(message_id = "MV4oVXSy", ?error, "failed to revert DNS: {}", error))
}

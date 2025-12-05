use crate::service::os::linux::positive_u31::PositiveU31;
use std::net::IpAddr;
use zbus_systemd::zbus;

async fn zbus_connect() -> Result<zbus_systemd::resolve1::ManagerProxy<'static>, ()> {
    let conn = zbus::Connection::system()
        .await
        .map_err(|error| tracing::error!(message_id = "SX4gJ91O", ?error, "failed to create DBUS system connection: {}", error))?;
    zbus_systemd::resolve1::ManagerProxy::new(&conn)
        .await
        .map_err(|error| tracing::error!(message_id = "AucCE8My", ?error, "to create resolved zbus proxy: {}", error))
        .map(|proxy| proxy.to_owned())
}

pub async fn set_dns(tun_idx: PositiveU31, dns: &[IpAddr]) -> Result<(), ()> {
    let dns = dns
        .iter()
        .map(|entry| match entry {
            IpAddr::V4(entry) => (libc::AF_INET, entry.octets().to_vec()),
            IpAddr::V6(entry) => (libc::AF_INET6, entry.octets().to_vec()),
        })
        .collect();
    // Equivalent to `resolvectl dns obscura <DNS IP>`
    zbus_connect()
        .await?
        .set_link_dns(tun_idx.into(), dns)
        .await
        .map_err(|error| tracing::error!(message_id = "H7vih0nS", ?error, "failed to set tun DNS IPs: {}", error))?;
    // Equivalent to `resolvectl domain obscuravpn ~.`. The `~` (or `true`) below, indicates a routing-only domain (not search domain)
    zbus_connect()
        .await?
        .set_link_domains(tun_idx.into(), vec![(".".to_string(), true)])
        .await
        .map_err(|error| tracing::error!(message_id = "92tR6ndT", ?error, "failed to set tun DNS domain: {}", error))?;
    Ok(())
}

pub async fn revert_dns(tun_idx: PositiveU31) -> Result<(), ()> {
    zbus_connect()
        .await?
        .revert_link(tun_idx.into())
        .await
        .map_err(|error| tracing::error!(message_id = "MV4oVXSy", ?error, "failed to revert DNS: {}", error))
}

use obscuravpn_client::net::NetworkInterface;
use semver::Version;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};

/// Minimum NetworkManager version required to use the preserve-external-ip flag in Device.Reapply. See https://networkmanager.dev/docs/api/latest/gdbus-org.freedesktop.NetworkManager.Device.html#gdbus-method-org-freedesktop-NetworkManager-Device.Reapply
const MIN_VERSION: Version = Version::new(1, 42, 0);

/// See https://networkmanager.dev/docs/api/latest/gdbus-org.freedesktop.NetworkManager.html
#[zbus::proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait NetworkManager {
    fn get_device_by_ip_iface(&self, iface: &str) -> zbus::Result<zbus::zvariant::OwnedObjectPath>;
    #[zbus(property)]
    fn version(&self) -> zbus::Result<String>;
}

/// See https://networkmanager.dev/docs/api/latest/gdbus-org.freedesktop.NetworkManager.Device.html
#[zbus::proxy(interface = "org.freedesktop.NetworkManager.Device", default_service = "org.freedesktop.NetworkManager")]
pub trait Device {
    fn get_applied_connection(&self, flags: u32) -> zbus::Result<(HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>>, u64)>;
    fn reapply(&self, connection: HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>>, version_id: u64, flags: u32) -> zbus::Result<()>;
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;
}

impl NetworkManagerProxy<'_> {
    async fn connect() -> Result<NetworkManagerProxy<'static>, ()> {
        let conn = zbus::Connection::system()
            .await
            .map_err(|error| tracing::error!(message_id = "xawuPraW", ?error, "failed to create DBUS system connection: {}", error))?;
        NetworkManagerProxy::new(&conn)
            .await
            .map_err(|error| tracing::error!(message_id = "glChFAF5", ?error, "failed to create network manager zbus proxy: {}", error))
            .map(|proxy| proxy.to_owned())
    }
    pub async fn device_proxy(self, interface: &NetworkInterface) -> Result<DeviceProxy<'static>, ()> {
        let device_path = self.get_device_by_ip_iface(&interface.name).await.map_err(|error| {
            tracing::error!(
                message_id = "WKUw8Oww",
                ?error,
                interface.name,
                "failed to get network manager device path: {}",
                error
            )
        })?;
        DeviceProxy::new(self.inner().connection(), device_path).await.map_err(|error| {
            tracing::error!(
                message_id = "aVyeWXo6",
                ?error,
                "failed to create network manager device proxy: {}",
                error
            )
        })
    }
}

/// Returns true if network manager is running and fulfills our minimal version requirement
pub async fn detect() -> bool {
    let Ok(proxy) = NetworkManagerProxy::connect().await else {
        return false;
    };
    match proxy.version().await {
        Ok(version) => {
            tracing::info!(message_id = "9nMAwOYu", version, "network manager is running");
            match Version::parse(&version) {
                Ok(version) => version >= MIN_VERSION,
                Err(error) => {
                    tracing::error!(message_id = "WKUw8Oww", ?error, "failed to parse network manager version: {}", error);
                    false
                }
            }
        }
        Err(error) => {
            tracing::error!(message_id = "WKUw8Oww", ?error, "failed to get network manager version: {}", error);
            false
        }
    }
}

pub async fn set_dns(tun: &NetworkInterface, dns: &[IpAddr]) -> Result<(), ()> {
    let proxy = NetworkManagerProxy::connect().await?.device_proxy(tun).await?;
    change_device_settings(&proxy, Some(dns)).await
}

pub async fn reset_dns(tun: &NetworkInterface) -> Result<(), ()> {
    let proxy = NetworkManagerProxy::connect().await?.device_proxy(tun).await?;

    let device_state = proxy.state().await.map_err(|error| {
        tracing::error!(
            message_id = "Yasp1leL",
            ?error,
            "failed to get current state from network manager device: {}",
            error
        )
    })?;

    // https://networkmanager.dev/docs/api/latest/nm-dbus-types.html#NMDeviceState
    tracing::info!(message_id = "hlyXxO69", device_state, "device state");
    if device_state == 10 {
        // network manager classifies new TUN devices without assigned IPs as "unmanaged" and refuses all device configuration interactions. If this happens, the TUN device is in the initial state, so no DNS config has been set yet and does not need to be reset.
        tracing::info!(
            message_id = "27mWEhLb",
            device_state,
            "network manager device unmanaged, skipping DNS reset"
        );
        return Ok(());
    }

    change_device_settings(&proxy, None).await
}

async fn change_device_settings(proxy: &DeviceProxy<'static>, dns: Option<&[IpAddr]>) -> Result<(), ()> {
    /// Setting this flag on Device.Reapply prevents removal of externally added IP addresses and routes. This does not seem to be respected if the ipv4 or ipv6 section of the applied settings are removed entirely or if the method is changed. See https://networkmanager.dev/docs/api/latest/gdbus-org.freedesktop.NetworkManager.Device.html#gdbus-method-org-freedesktop-NetworkManager-Device.Reapply
    const PRESERVE_EXTERNAL_IP: u32 = 0x1;

    let (old_settings, _) = proxy.get_applied_connection(0).await.map_err(|error| {
        tracing::error!(
            message_id = "VnZD0s3r",
            ?error,
            "failed to get current config from network manager device: {}",
            error
        )
    })?;

    let settings = build_device_settings(old_settings, dns).map_err(|error| {
        tracing::error!(
            message_id = "jj3NwH49",
            ?error,
            "failed to change network manager DNS settings: {}",
            error
        )
    })?;

    proxy.reapply(settings, 0, PRESERVE_EXTERNAL_IP).await.map_err(|error| {
        tracing::error!(
            message_id = "EgcIC6PF",
            ?error,
            "failed to apply DNS config changes to network manager device: {}",
            error
        )
    })
}

fn build_device_settings(
    mut old_settings: HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>>,
    dns: Option<&[IpAddr]>,
) -> Result<HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>>, zbus::zvariant::Error> {
    // See
    // - https://networkmanager.dev/docs/api/latest/nm-settings-nmcli.html
    // - https://networkmanager.dev/docs/api/1.44.4/NetworkManager.conf.html
    // for (incomplete) lists of supported properties.

    use zbus::zvariant::{Str, Value};

    let method = Str::from_static("manual");
    let mut dns_search = vec![];
    let mut dns_addresses_v4 = vec![];
    let mut dns_addresses_v6 = vec![];

    if let Some(dns) = dns {
        dns_search = vec!["~"];
        for dns_ip in dns {
            match dns_ip {
                IpAddr::V4(dns_ip) => dns_addresses_v4.push(ipv4_to_u32(*dns_ip)),
                IpAddr::V6(dns_ip) => dns_addresses_v6.push(dns_ip.octets().to_vec()),
            }
        }
    };

    let mut ipv4_settings = HashMap::from([
        ("method".into(), method.clone().into()),
        ("dns-priority".into(), i32::MIN.into()),
        ("dns-search".into(), Value::from(&dns_search).try_into()?),
        ("dns".into(), Value::from(dns_addresses_v4).try_into()?),
    ]);
    move_hashmap_items(old_settings.get_mut("ipv4"), &mut ipv4_settings, &["addresses"]);

    let mut ipv6_settings = HashMap::from([
        ("method".into(), method.into()),
        ("dns-priority".into(), i32::MIN.into()),
        ("dns-search".into(), Value::from(dns_search).try_into()?),
        ("dns".into(), Value::from(dns_addresses_v6).try_into()?),
    ]);
    move_hashmap_items(old_settings.get_mut("ipv6"), &mut ipv6_settings, &["addresses"]);

    let mut settings = HashMap::from([("ipv4".into(), ipv4_settings), ("ipv6".into(), ipv6_settings)]);
    move_hashmap_items(&mut old_settings, &mut settings, &["connection"]);

    Ok(settings)
}

fn ipv4_to_u32(ip: Ipv4Addr) -> u32 {
    // NetworkManager IPs are the octets as u32, in their original byte order.
    u32::from_ne_bytes(ip.octets())
}

fn move_hashmap_items<'a, V: 'static>(src: impl Into<Option<&'a mut HashMap<String, V>>>, dest: &mut HashMap<String, V>, keys: &[&str]) {
    if let Some(src) = src.into() {
        for key in keys {
            if let Some((key, value)) = src.remove_entry(*key) {
                dest.insert(key, value);
            }
        }
    }
}

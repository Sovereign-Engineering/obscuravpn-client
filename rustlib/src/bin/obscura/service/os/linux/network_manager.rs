use ipnetwork::IpNetwork;
use obscuravpn_client::net::NetworkInterface;
use obscuravpn_client::network_config::TunnelNetworkConfig;
use semver::Version;
use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr};

/// Minimum supported NetworkManager version. Some NetworkManager versions behave in various unexpected ways, especially with respect to IPv6 route maintenance.
/// Version 1.42.4 on Debian 12 both clears externally set IPv6 routes (despite preserve-external-ip flag) and does not apply provided IPv6 routes.
/// The first version that is confirmed to correctly interact with externally managed routes (with the preserve-external-ip flag) is 1.54.0-2.fc43 (tested on Fedora 43).
const MIN_VERSION: Version = Version::new(1, 52, 1);

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
    async fn connect() -> Result<(NetworkManagerProxy<'static>, Version), ()> {
        let conn = zbus::Connection::system()
            .await
            .map_err(|error| tracing::error!(message_id = "xawuPraW", ?error, "failed to create DBUS system connection: {}", error))?;
        let proxy = NetworkManagerProxy::new(&conn)
            .await
            .map_err(|error| tracing::error!(message_id = "glChFAF5", ?error, "failed to create network manager zbus proxy: {}", error))
            .map(|proxy| proxy.to_owned())?;
        let version = proxy
            .version()
            .await
            .map_err(|error| tracing::error!(message_id = "WKUw8Oww", ?error, "failed to get network manager version: {}", error))?;
        let version = Version::parse(&version)
            .map_err(|error| tracing::error!(message_id = "WKUw8Oww", ?error, "failed to parse network manager version: {}", error))?;
        Ok((proxy, version))
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
    let Ok((_proxy, version)) = NetworkManagerProxy::connect().await else {
        return false;
    };
    tracing::info!(message_id = "9nMAwOYu", %version, "network manager is running");
    version >= MIN_VERSION
}

pub async fn set_dns_and_routes(tun: &NetworkInterface, network_config: &TunnelNetworkConfig, routes: &[IpNetwork]) -> Result<(), ()> {
    let (nm_proxy, _nm_version) = NetworkManagerProxy::connect().await?;
    let proxy = nm_proxy.device_proxy(tun).await?;
    apply_device_settings(tun, &proxy, network_config, routes, true).await
}

pub async fn reset_dns_and_routes(tun: &NetworkInterface) -> Result<(), ()> {
    let (nm_proxy, _nm_version) = NetworkManagerProxy::connect().await?;
    let proxy = nm_proxy.device_proxy(tun).await?;
    let network_config = TunnelNetworkConfig::dummy();
    apply_device_settings(tun, &proxy, &network_config, &[], false).await
}

async fn apply_device_settings(
    tun: &NetworkInterface,
    proxy: &DeviceProxy<'static>,
    network_config: &TunnelNetworkConfig,
    routes: &[IpNetwork],
    enable_dns: bool,
) -> Result<(), ()> {
    /// Setting this flag on Device.Reapply prevents removal of externally added IP addresses and routes. This does not seem to be respected if the ipv4 or ipv6 section of the applied settings are removed entirely or if the method is changed. See https://networkmanager.dev/docs/api/latest/gdbus-org.freedesktop.NetworkManager.Device.html#gdbus-method-org-freedesktop-NetworkManager-Device.Reapply
    /// Some versions of NetworkManager remove all IPs and move the device to unmanaged otherwise (e.g. 1.52.1 on Debian 13).
    const PRESERVE_EXTERNAL_IP: u32 = 0x1;

    let settings = build_device_settings(tun, network_config, routes, enable_dns).map_err(|error| {
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
    tun: &NetworkInterface,
    network_config: &TunnelNetworkConfig,
    routes: &[IpNetwork],
    enable_dns: bool,
) -> Result<HashMap<String, HashMap<String, zbus::zvariant::OwnedValue>>, zbus::zvariant::Error> {
    // See
    // - https://networkmanager.dev/docs/api/latest/nm-settings-nmcli.html
    // - https://networkmanager.dev/docs/api/1.44.4/NetworkManager.conf.html
    // for (incomplete) lists of supported properties.

    use zbus::zvariant::{Str, Value};

    let method = Str::from_static("manual");

    let ipv4_address_data: Vec<HashMap<String, zbus::zvariant::OwnedValue>> = vec![HashMap::from([
        ("address".into(), Value::from(network_config.ipv4.to_string()).try_into()?),
        ("prefix".into(), Value::from(32u32).try_into()?),
    ])];

    let ipv4_route_data: Vec<HashMap<String, zbus::zvariant::OwnedValue>> = routes
        .iter()
        .cloned()
        .filter(IpNetwork::is_ipv4)
        .map(route_to_dbus_hashmap)
        .collect::<Result<Vec<_>, _>>()?;

    let mut ipv4_settings = HashMap::from([
        ("address-data".into(), Value::from(ipv4_address_data).try_into()?),
        ("route-data".into(), Value::from(ipv4_route_data).try_into()?),
        ("method".into(), method.clone().into()),
        ("never-default".into(), true.into()),
        ("may-fail".into(), true.into()),
    ]);

    let ipv6_address_data: Vec<HashMap<String, zbus::zvariant::OwnedValue>> = vec![HashMap::from([
        ("address".into(), Value::from(network_config.ipv6.ip().to_string()).try_into()?),
        ("prefix".into(), Value::from(u32::from(network_config.ipv6.prefix())).try_into()?),
    ])];

    let ipv6_route_data: Vec<HashMap<String, zbus::zvariant::OwnedValue>> = routes
        .iter()
        .cloned()
        .filter(IpNetwork::is_ipv6)
        .map(route_to_dbus_hashmap)
        .collect::<Result<Vec<_>, _>>()?;

    let mut ipv6_settings = HashMap::from([
        ("address-data".into(), Value::from(ipv6_address_data).try_into()?),
        ("route-data".into(), Value::from(ipv6_route_data).try_into()?),
        ("method".into(), method.into()),
        ("never-default".into(), true.into()),
        ("may-fail".into(), true.into()),
    ]);

    let connection_settings = HashMap::from([
        ("type".into(), Str::from_static("tun").into()),
        ("id".into(), Value::from(&tun.name).try_into()?),
        ("interface-name".into(), Value::from(&tun.name).try_into()?),
        ("autoconnect".into(), true.into()),
    ]);

    //  NetworkManager 1.52.1 on Debian 13 will generate an empty /etc/resolv.conf if these settings are specified (after previously applying a non-empty tunnel DNS configuration correctly), but don't contain any DNS server addresses. Both some older and newer versions do not have this problem.
    if enable_dns {
        let dns_search = vec!["~"];
        let mut dns_addresses_v4 = vec![];
        let mut dns_addresses_v6 = vec![];
        for dns_ip in &network_config.dns {
            match dns_ip {
                IpAddr::V4(dns_ip) => dns_addresses_v4.push(ipv4_to_u32(*dns_ip)),
                IpAddr::V6(dns_ip) => dns_addresses_v6.push(dns_ip.octets().to_vec()),
            }
        }
        for ipvx_settings in [&mut ipv4_settings, &mut ipv6_settings] {
            ipvx_settings.insert("dns-priority".into(), i32::MIN.into());
            ipvx_settings.insert("dns-search".into(), Value::from(&dns_search).try_into()?);
        }
        ipv4_settings.insert("dns".into(), Value::from(dns_addresses_v4).try_into()?);
        ipv6_settings.insert("dns".into(), Value::from(dns_addresses_v6).try_into()?);
    }

    Ok(HashMap::from([
        ("connection".into(), connection_settings),
        ("ipv4".into(), ipv4_settings),
        ("ipv6".into(), ipv6_settings),
    ]))
}

fn ipv4_to_u32(ip: Ipv4Addr) -> u32 {
    // NetworkManager IPs are the octets as u32, in their original byte order.
    u32::from_ne_bytes(ip.octets())
}

fn route_to_dbus_hashmap(net: IpNetwork) -> Result<HashMap<String, zbus::zvariant::OwnedValue>, zbus::zvariant::Error> {
    Ok(HashMap::from([
        ("dest".into(), zbus::zvariant::Value::from(net.ip().to_string()).try_into()?),
        ("prefix".into(), u32::from(net.prefix()).into()),
    ]))
}

use ipnetwork::{IpNetwork, Ipv4Network, Ipv6Network};
use obscuravpn_client::os::windows::adapters::list_network_adapters;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use windows::Win32::Foundation::{ERROR_NOT_FOUND, ERROR_OBJECT_ALREADY_EXISTS, NO_ERROR, WIN32_ERROR};
use windows::Win32::NetworkManagement::IpHelper::{CreateIpForwardEntry2, DeleteIpForwardEntry2, InitializeIpForwardEntry, MIB_IPFORWARD_ROW2};
use windows::Win32::NetworkManagement::IpHelper::{
    DNS_INTERFACE_SETTINGS, DNS_INTERFACE_SETTINGS_VERSION1, DNS_SETTING_NAMESERVER, SetInterfaceDnsSettings,
};
use windows::Win32::NetworkManagement::IpHelper::{GetIpInterfaceEntry, MIB_IPINTERFACE_ROW, SetIpInterfaceEntry};
use windows::Win32::NetworkManagement::Ndis::NET_LUID_LH;
use windows::Win32::Networking::WinSock::{ADDRESS_FAMILY, AF_INET, AF_INET6, IN_ADDR, IN6_ADDR, IN6_ADDR_0, SOCKADDR_IN, SOCKADDR_IN6};
use windows::Win32::UI::Shell::SHGetKnownFolderPath;

/// The individual routes cover half of the respective address space, which gives them priority over the default route without replacing it. We don't want to replace the default route, because:
/// - We use the default route for preferred network interface discovery
/// - We wouldn't know what the set it to when the tunnel is disabled
/// - Network management services tend to overwrite it.
const ROUTES: [IpNetwork; 4] = [
    IpNetwork::V4(Ipv4Network::new_checked(Ipv4Addr::new(000, 0, 0, 0), 1).unwrap()),
    IpNetwork::V4(Ipv4Network::new_checked(Ipv4Addr::new(128, 0, 0, 0), 1).unwrap()),
    IpNetwork::V6(Ipv6Network::new_checked(Ipv6Addr::new(0x0000, 0, 0, 0, 0, 0, 0, 0), 1).unwrap()),
    IpNetwork::V6(Ipv6Network::new_checked(Ipv6Addr::new(0x8000, 0, 0, 0, 0, 0, 0, 0), 1).unwrap()),
];

/// Excludes IPv6 addresses when IPv6 is disabled.
fn tunnel_routes(dns: &[IpAddr], ipv6: bool) -> impl Iterator<Item = IpNetwork> {
    ROUTES
        .into_iter()
        .chain(dns.iter().copied().map(IpNetwork::from))
        .filter(move |route| ipv6 || route.is_ipv4())
}

pub fn set_routes(adapter: &wintun::Adapter, dns: &[IpAddr], stale_dns: &[IpAddr], ipv6: bool) -> Result<(), ()> {
    let if_index = adapter
        .get_adapter_index()
        .map_err(|error| tracing::error!(message_id = "Xt7kR2mN", ?error, "failed to get adapter index for setting routes"))?;

    let mut result = Ok(());
    for route in tunnel_routes(dns, ipv6) {
        result = result.and(add_route(if_index, &route));
    }
    for ip in stale_dns.iter().filter(|ip| ipv6 || ip.is_ipv4()) {
        result = result.and(remove_route(if_index, &IpNetwork::from(*ip)));
    }
    result
}

pub fn remove_routes(adapter: &wintun::Adapter, dns: &[IpAddr], ipv6: bool) -> Result<(), ()> {
    let if_index = adapter
        .get_adapter_index()
        .map_err(|error| tracing::error!(message_id = "Yt8lS3nP", ?error, "failed to get adapter index for removing routes"))?;

    let mut result = Ok(());
    for route in tunnel_routes(dns, ipv6) {
        result = result.and(remove_route(if_index, &route));
    }
    result
}

/// Whether an adapter other than `exclude_luid` has an IPv6 default gateway, i.e. whether IPv6 traffic could bypass the tunnel.
pub fn has_ipv6_default_route(exclude_luid: u64) -> Result<bool, ()> {
    let adapters = list_network_adapters().map_err(|error| tracing::error!(message_id = "Hx7pB2mW", ?error, "GetAdaptersAddresses failed"))?;
    Ok(adapters
        .iter()
        .any(|adapter| adapter.luid != exclude_luid && adapter.gateways.iter().any(IpAddr::is_ipv6)))
}

/// Build a `MIB_IPFORWARD_ROW2` for the given interface, destination, and prefix length.
/// The next hop is set to unspecified (all zeros) — traffic goes directly to the tunnel interface.
fn build_forward_row(if_index: u32, dest: IpAddr, prefix_len: u8) -> MIB_IPFORWARD_ROW2 {
    let mut row = MIB_IPFORWARD_ROW2::default();
    // SAFETY: `row` is an OUT param; zeroed defensively.
    unsafe { InitializeIpForwardEntry(&mut row) };

    row.InterfaceIndex = if_index;
    row.ValidLifetime = u32::MAX;
    row.PreferredLifetime = u32::MAX;
    row.DestinationPrefix.PrefixLength = prefix_len;

    match dest {
        IpAddr::V4(addr) => {
            row.DestinationPrefix.Prefix.Ipv4 = make_sockaddr_in(addr);
            row.NextHop.Ipv4 = make_sockaddr_in(Ipv4Addr::UNSPECIFIED);
        }
        IpAddr::V6(addr) => {
            row.DestinationPrefix.Prefix.Ipv6 = make_sockaddr_in6(addr);
            row.NextHop.Ipv6 = make_sockaddr_in6(Ipv6Addr::UNSPECIFIED);
        }
    }

    row
}

fn add_route(if_index: u32, route: &IpNetwork) -> Result<(), ()> {
    let row = build_forward_row(if_index, route.ip(), route.prefix());
    // SAFETY: `row` is a valid, fully initialized `MIB_IPFORWARD_ROW2` built by
    // `build_forward_row`. `CreateIpForwardEntry2` only reads the pointed-to struct.
    let win32_error = unsafe { CreateIpForwardEntry2(&row) };
    if win32_error.is_ok() {
        tracing::info!(message_id = "VoLYoYNn", route = %route, "added route");
        Ok(())
    } else if win32_error == ERROR_OBJECT_ALREADY_EXISTS {
        Ok(())
    } else {
        tracing::error!(
            message_id = "Qv3bW8cJ",
            ?win32_error,
            route = %route,
            "failed to add route"
        );
        Err(())
    }
}

fn remove_route(if_index: u32, route: &IpNetwork) -> Result<(), ()> {
    let row = build_forward_row(if_index, route.ip(), route.prefix());
    // SAFETY: `row` is a valid, fully initialized `MIB_IPFORWARD_ROW2` built by
    // `build_forward_row`. `DeleteIpForwardEntry2` only reads the pointed-to struct.
    let win32_error = unsafe { DeleteIpForwardEntry2(&row) };
    if win32_error.is_ok() {
        tracing::info!(message_id = "UUNppVZg", route = %route, "removed route");
        Ok(())
    } else if win32_error == ERROR_NOT_FOUND {
        Ok(())
    } else {
        tracing::error!(
            message_id = "Rv4cW9dK",
            ?win32_error,
            route = %route,
            "failed to remove route"
        );
        Err(())
    }
}

fn make_sockaddr_in(addr: Ipv4Addr) -> SOCKADDR_IN {
    SOCKADDR_IN {
        sin_family: ADDRESS_FAMILY(AF_INET.0),
        sin_addr: IN_ADDR { S_un: windows::Win32::Networking::WinSock::IN_ADDR_0 { S_addr: addr.to_bits().to_be() } },
        ..Default::default()
    }
}

fn make_sockaddr_in6(addr: Ipv6Addr) -> SOCKADDR_IN6 {
    SOCKADDR_IN6 {
        sin6_family: ADDRESS_FAMILY(AF_INET6.0),
        sin6_addr: IN6_ADDR { u: IN6_ADDR_0 { Byte: addr.octets() } },
        ..Default::default()
    }
}

fn family_name(family: ADDRESS_FAMILY) -> &'static str {
    if family == AF_INET {
        "IPv4"
    } else if family == AF_INET6 {
        "IPv6"
    } else {
        "unknown"
    }
}

fn get_ip_interface_entry(adapter: &wintun::Adapter, family: ADDRESS_FAMILY) -> Result<MIB_IPINTERFACE_ROW, WIN32_ERROR> {
    let mut row = MIB_IPINTERFACE_ROW {
        Family: family,
        // SAFETY: Accessing `Value` field of a union to copy the raw 64-bit LUID
        InterfaceLuid: NET_LUID_LH { Value: unsafe { adapter.get_luid().Value } },
        ..Default::default()
    };
    // SAFETY: `row` is a properly initialized `MIB_IPINTERFACE_ROW` with `Family` and
    // `InterfaceLuid` set. `GetIpInterfaceEntry` reads those fields and fills the rest.
    let result = unsafe { GetIpInterfaceEntry(&mut row) };
    if result == NO_ERROR { Ok(row) } else { Err(result) }
}

/// The adapter has no IPv6 interface when IPv6 is disabled on the host or unbound from the adapter.
pub fn has_ipv6_interface(adapter: &wintun::Adapter) -> bool {
    match get_ip_interface_entry(adapter, AF_INET6) {
        Ok(_) => true,
        Err(error) if error == ERROR_NOT_FOUND => false,
        Err(error) => {
            tracing::error!(
                message_id = "Vq3nJt8L",
                error_code = error.0,
                "GetIpInterfaceEntry failed for IPv6, assuming it's enabled"
            );
            true
        }
    }
}

fn set_metric(adapter: &wintun::Adapter, automatic: bool, metric: u32, ipv6: bool) -> Result<(), ()> {
    let families: &[ADDRESS_FAMILY] = if ipv6 { &[AF_INET, AF_INET6] } else { &[AF_INET] };
    let mut success = Ok(());
    for &family in families {
        let mut row = match get_ip_interface_entry(adapter, family) {
            Ok(row) => row,
            Err(error) => {
                tracing::error!(
                    message_id = "nVRsbT3w",
                    error_code = error.0,
                    family = family_name(family),
                    "GetIpInterfaceEntry failed"
                );
                success = Err(());
                continue;
            }
        };

        // https://learn.microsoft.com/windows/win32/api/netioapi/ns-netioapi-mib_ipinterface_row
        // For IPv4, SitePrefixLength is set to 64 by GetIpInterfaceEntry.
        // This is illegal as the max value is 32 for IPv4.
        // This hiccup has been documented by Wireguard and on StackOverflow
        // https://github.com/WireGuard/wireguard-windows/blob/0f52c8d37528e2a768a2f63472656bc93bc4546f/tunnel/winipcfg/types.go#L666C5-L666C114
        // For IPv4, SitePrefixLength "must be set to 0".
        row.SitePrefixLength = if family == AF_INET { 0 } else { 128 };
        row.UseAutomaticMetric = automatic;
        row.Metric = metric;

        // SAFETY: `row` was successfully populated by `GetIpInterfaceEntry` and we have only
        // modified documented fields. `SetIpInterfaceEntry` writes the updated row back.
        // https://learn.microsoft.com/windows/win32/api/netioapi/nf-netioapi-setipinterfaceentry#remarks
        let result = unsafe { SetIpInterfaceEntry(&mut row) };
        if result != NO_ERROR {
            tracing::error!(
                message_id = "Viabd7Fj",
                interfaceLuid = unsafe { row.InterfaceLuid.Value },
                error_code = result.0,
                family = family_name(family),
                metric,
                "SetIpInterfaceEntry failed"
            );
            success = Err(());
        }
    }
    success
}

pub fn set_low_metric(adapter: &wintun::Adapter, ipv6: bool) -> Result<(), ()> {
    set_metric(adapter, false, 1, ipv6)?;
    tracing::info!(message_id = "frfGU26w", "Successfully set interface metric to 1");
    Ok(())
}

pub fn reset_interface_metric(adapter: &wintun::Adapter, ipv6: bool) -> Result<(), ()> {
    set_metric(adapter, true, 0, ipv6)?;
    tracing::info!(message_id = "5EdQ1ti3", "Successfully reset interface metric to automatic");
    Ok(())
}

fn get_system_directory() -> std::path::PathBuf {
    // SAFETY: `SHGetKnownFolderPath` is called with valid parameters. The returned PWSTR
    // is a COM-allocated wide string that we convert and then free with `CoTaskMemFree`.
    let result = unsafe {
        SHGetKnownFolderPath(
            &windows::Win32::UI::Shell::FOLDERID_System,
            windows::Win32::UI::Shell::KNOWN_FOLDER_FLAG::default(),
            None,
        )
    };
    if let Ok(pwstr) = result {
        let wide = unsafe { pwstr.to_string() };
        unsafe { windows::Win32::System::Com::CoTaskMemFree(Some(pwstr.0.cast_const().cast())) };
        if let Ok(s) = wide {
            return std::path::PathBuf::from(s);
        }
    }
    tracing::warn!(message_id = "Jw2xN5qR", "SHGetKnownFolderPath failed, falling back to SystemRoot env");
    std::path::PathBuf::from(std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".to_string())).join("System32")
}

async fn run_command(cmd: &mut tokio::process::Command, friendly_name: &str, message_id: &str) -> Result<(), ()> {
    let output = cmd.output().await.map_err(|error| {
        tracing::error!(message_id = "tFgqHB7v", ?error, friendly_name, "failed to spawn command");
    })?;
    if output.status.success() {
        Ok(())
    } else {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        tracing::error!(message_id = message_id, %stdout, %stderr, friendly_name, "command failed");
        Err(())
    }
}

pub async fn set_ipv4_address(adapter: &wintun::Adapter, ipv4: Ipv4Addr) -> Result<(), ()> {
    let name = adapter
        .get_name()
        .map_err(|error| tracing::error!(message_id = "fUqTnRC8", ?error, "failed to get adapter name"))?;
    let netsh = get_system_directory().join("netsh.exe");
    let mut cmd = tokio::process::Command::new(&netsh);
    cmd.args(["interface", "ipv4", "set", "address", &name, "source=static"])
        .arg(format!("address={ipv4}"))
        .arg("mask=255.255.255.255");
    run_command(&mut cmd, "netsh for IPv4", "2Yndy7Y5").await
}

pub async fn set_ipv6_address(adapter: &wintun::Adapter, ipv6: Ipv6Network) -> Result<(), ()> {
    let name = adapter
        .get_name()
        .map_err(|error| tracing::error!(message_id = "k3mRv8wQ", ?error, "failed to get adapter name"))?;
    let netsh = get_system_directory().join("netsh.exe");
    let mut cmd = tokio::process::Command::new(&netsh);
    cmd.args(["interface", "ipv6", "set", "address", &name])
        .arg(format!("address={}/{}", ipv6.ip(), ipv6.prefix()));
    run_command(&mut cmd, "netsh for IPv6", "p7nWx2kF").await
}

pub async fn set_mtu(adapter: &wintun::Adapter, mtu: u16, ipv6: bool) -> Result<(), ()> {
    let name = adapter
        .get_name()
        .map_err(|error| tracing::error!(message_id = "gHUMlkA6", ?error, "failed to get adapter name for MTU"))?;
    let netsh = get_system_directory().join("netsh.exe");

    let families: &[&str] = if ipv6 { &["ipv4", "ipv6"] } else { &["ipv4"] };
    let mut result = Ok(());
    for &ip_str in families {
        let mut cmd = tokio::process::Command::new(&netsh);
        cmd.args(["interface", ip_str, "set", "subinterface", &name])
            .arg(format!("mtu={mtu}"))
            .arg("store=persistent");
        tracing::info!(message_id = "HJLqy3YD", mtu, "setting mtu via netsh");
        result = result.and(run_command(&mut cmd, "netsh set MTU", "2NrLhFYu").await);
    }
    result
}

/// Set DNS servers on the adapter.
/// First attempts the Windows API (`SetInterfaceDnsSettings`); Falls back to netsh.
pub async fn set_dns_servers(adapter: &wintun::Adapter, dns: &[IpAddr]) -> Result<(), ()> {
    let guid = windows::core::GUID::from_u128(adapter.get_guid());
    if let Err(error) = set_interface_dns_settings(guid, dns) {
        tracing::warn!(message_id = "OhrWluk1", ?error, "SetInterfaceDnsSettings failed, falling back to netsh");
        set_dns_servers_netsh(adapter, dns).await?;
    }
    Ok(())
}

// Available: Windows 10 Build 19041
fn set_interface_dns_settings(interface: windows::core::GUID, dns: &[IpAddr]) -> Result<(), ()> {
    let dns_str: String = dns.iter().map(|ip| ip.to_string()).collect::<Vec<_>>().join(",");
    let dns_wide: Vec<u16> = dns_str.encode_utf16().chain(std::iter::once(0)).collect();

    let settings = DNS_INTERFACE_SETTINGS {
        Version: DNS_INTERFACE_SETTINGS_VERSION1,
        Flags: u64::from(DNS_SETTING_NAMESERVER),
        NameServer: windows::core::PWSTR(dns_wide.as_ptr().cast_mut()),
        ..Default::default()
    };

    // SAFETY: `interface` is a valid GUID from the adapter. `settings` is a properly initialized
    // `DNS_INTERFACE_SETTINGS` with a valid null-terminated UTF-16 `NameServer` pointer.
    let result = unsafe { SetInterfaceDnsSettings(interface, &settings) };
    if result == NO_ERROR {
        Ok(())
    } else {
        tracing::error!(message_id = "60E01Rf2", error_code = result.0, "SetInterfaceDnsSettings failed");
        Err(())
    }
}

async fn set_dns_servers_netsh(adapter: &wintun::Adapter, dns: &[IpAddr]) -> Result<(), ()> {
    let mut result = Ok(());

    let name = adapter
        .get_name()
        .map_err(|error| tracing::error!(message_id = "MxyYv5ln", ?error, "failed to get adapter name for DNS"))?;
    let netsh = get_system_directory().join("netsh.exe");

    if dns.is_empty() {
        for ip_str in ["ipv4", "ipv6"] {
            let mut cmd = tokio::process::Command::new(&netsh);
            cmd.args(["interface", ip_str, "set", "dnsservers", &name, "static", "none", "register=both"]);
            result = result.and(run_command(&mut cmd, "netsh set dns servers none", "YpBklPNr").await);
        }
        return result;
    }

    let ip_str = if dns[0].is_ipv4() { "ipv4" } else { "ipv6" };

    let mut cmd = tokio::process::Command::new(&netsh);
    cmd.args(["interface", ip_str, "set", "dnsservers", &name, "static"])
        .arg(format!("{}", dns[0]))
        .arg("register=both")
        .arg("validate=no");
    result = result.and(run_command(&mut cmd, "netsh set dns servers", "jYNNDpN6").await);

    for nameserver in dns.iter().skip(1) {
        let ip_str = if nameserver.is_ipv4() { "ipv4" } else { "ipv6" };
        let mut cmd = tokio::process::Command::new(&netsh);
        cmd.args(["interface", ip_str, "add", "dnsservers", &name])
            .arg(nameserver.to_string())
            .arg("validate=no");
        result = result.and(run_command(&mut cmd, "netsh add dns server", "920fvXuP").await);
    }

    result
}

pub async fn flush_dns_cache() -> Result<(), ()> {
    let ipconfig = get_system_directory().join("ipconfig.exe");
    let mut cmd = tokio::process::Command::new(&ipconfig);
    cmd.arg("/flushdns");
    run_command(&mut cmd, "ipconfig /flushdns", "Gx5tL9nB").await?;
    tracing::info!(message_id = "SnUcIehf", "successfully flushed DNS resolver cache");
    Ok(())
}

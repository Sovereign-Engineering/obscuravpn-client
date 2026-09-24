use obscuravpn_client::net::NetworkInterface;
use obscuravpn_client::os::windows::adapters::{NetworkAdapter, list_network_adapters};
use obscuravpn_client::positive_u31::PositiveU31;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;
use tokio::sync::watch::{Receiver, Sender, channel};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::{ERROR_IO_PENDING, HANDLE};
use windows::Win32::NetworkManagement::IpHelper::{
    CancelIPChangeNotify, IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211, IF_TYPE_WWANPP, IF_TYPE_WWANPP2, NotifyAddrChange,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::System::IO::OVERLAPPED;
use windows::Win32::System::Threading::{CreateEventW, INFINITE, ResetEvent, WaitForSingleObject};

const WATCH_ERROR_BACKOFF: Duration = Duration::from_secs(1);

/// `tun_luid` identifies our own tunnel adapter, which is never selected.
pub fn watch_active_adapter(tun_luid: u64) -> Receiver<Option<NetworkInterface>> {
    let (sender, receiver) = channel(None);
    std::thread::spawn(move || watch_active_adapter_thread(&sender, tun_luid));
    receiver
}

fn watch_active_adapter_thread(sender: &Sender<Option<NetworkInterface>>, tun_luid: u64) {
    loop {
        watch_addr_changes(sender, tun_luid);
        std::thread::sleep(WATCH_ERROR_BACKOFF);
    }
}

/// Creates an event handle, watches for address changes, and cleans up. Returns when an error occurs.
fn watch_addr_changes(sender: &Sender<Option<NetworkInterface>>, tun_luid: u64) {
    // SAFETY: `CreateEventW` with all-default/null parameters creates an anonymous,
    // manual-reset event. No unsafe preconditions beyond a valid call.
    let event_handle = match unsafe { CreateEventW(None, true, false, None) } {
        Err(error) => {
            tracing::error!(message_id = "aB3kW9xP", ?error, "CreateEventW failed");
            return;
        }
        Ok(event_handle) => event_handle,
    };

    let overlapped = OVERLAPPED { hEvent: event_handle, ..Default::default() };
    let mut notify_handle = HANDLE::default();

    loop {
        // SAFETY: `event_handle` is a valid event handle created by `CreateEventW`.
        if let Err(error) = unsafe { ResetEvent(event_handle) } {
            tracing::error!(message_id = "pvY8miRB", ?error, "ResetEvent failed");
            break;
        }

        // SAFETY: `overlapped.hEvent` is the same valid event handle. `notify_handle` is
        // an out-parameter that receives the notification handle; "Warning Do not close this handle"
        let ret = unsafe { NotifyAddrChange(&mut notify_handle, &overlapped) };
        // The return value would only be zero if both params are NULL
        if ret != ERROR_IO_PENDING.0 {
            tracing::error!(message_id = "x0HwRYyz", ret, "NotifyAddrChange failed");
            break;
        }

        // Get adapter AFTER subscribing but before waiting to avoid race conditions
        let Ok(adapter) = get_active_adapter(tun_luid) else {
            // SAFETY: overlapped is on the stack
            if !unsafe { CancelIPChangeNotify(&overlapped) }.as_bool() {
                // Should not occur. Indicates missing notification, invalid overlapped, or
                // insufficient error handling of NotifyAddrChange
                tracing::error!(message_id = "1uP30TS8", "could not deregister change notification");
            }
            break;
        };
        sender.send_if_modified(|current| {
            if *current != adapter {
                tracing::info!(message_id = "3vxyU7ra", ?current, ?adapter, "preferred network interface changed");
                *current = adapter;
                true
            } else {
                false
            }
        });

        // SAFETY: `event_handle` is a valid event handle;
        // `INFINITE` timeout means this blocks until the event is signalled by `NotifyAddrChange`.
        let event = unsafe { WaitForSingleObject(event_handle, INFINITE) };
        if event.0 != 0 {
            tracing::error!(message_id = "538dQYke", event = event.0, "WaitForSingleObject failed");
            break;
        }
    }

    // SAFETY: `event_handle` is the valid handle `CreateEventW` returned above and this is its only close.
    if let Err(error) = unsafe { CloseHandle(event_handle) } {
        tracing::warn!(message_id = "oLmLePMW", ?error, "failed to close event handle");
    }
}

fn get_active_adapter(tun_luid: u64) -> Result<Option<NetworkInterface>, ()> {
    let mut adapters = list_network_adapters().map_err(|error| tracing::error!(message_id = "GPNijM7d", ?error, "GetAdaptersAddresses failed"))?;
    adapters.retain(|adapter| adapter.luid != tun_luid);
    let Some((adapter, ip)) = select_adapter(&adapters) else {
        tracing::info!(message_id = "E7scsGZH", "did not find an active adapter");
        return Ok(None);
    };
    let index = PositiveU31::try_from(adapter.index).map_err(|error| {
        tracing::error!(
            message_id = "KRgY0doM",
            ?error,
            index = adapter.index,
            "adapter index out of range for PositiveU31"
        )
    })?;
    let mtu: i32 = adapter
        .mtu
        .try_into()
        .map_err(|error| tracing::error!(message_id = "TDYf7bGF", ?error, mtu = adapter.mtu, "adapter MTU out of range for i32"))?;
    Ok(Some(NetworkInterface {
        name: adapter.friendly_name.clone(),
        index,
        ip: IpAddr::V4(ip),
        mtu,
    }))
}

/// Candidates are up, have an IPv4 default gateway, an IPv4 address, and are ordered by interface metric.
/// Tunnel is currently IPv4-only, so we need the adapter that handles IPv4 addresses.
/// Prefer a hardware (i.e. ethernet, wifi, cellular) adapter, otherwise fall back to the best remaining candidate.
fn select_adapter(adapters: &[NetworkAdapter]) -> Option<(&NetworkAdapter, Ipv4Addr)> {
    let mut candidates: Vec<(&NetworkAdapter, Ipv4Addr)> = adapters
        .iter()
        .filter(|adapter| adapter.oper_status == IfOperStatusUp.0 && adapter.gateways.iter().any(IpAddr::is_ipv4))
        .filter_map(|adapter| Some((adapter, adapter.ipv4_address?)))
        .collect();
    candidates.sort_by_key(|(adapter, _)| adapter.ipv4_metric);
    if let Some(physical) = candidates.iter().find(|(adapter, _)| is_physical(adapter)) {
        return Some(*physical);
    }
    let (fallback, ip) = *candidates.first()?;
    tracing::warn!(
        message_id = "7nAgVt8N",
        name = %fallback.friendly_name,
        if_type = fallback.if_type,
        hardware = ?fallback.hardware,
        "no hardware ethernet/wifi/cellular adapter has a default route, falling back to the lowest-metric adapter that is up and isn't our tunnel"
    );
    Some((fallback, ip))
}

fn is_physical(adapter: &NetworkAdapter) -> bool {
    matches!(
        adapter.if_type,
        IF_TYPE_ETHERNET_CSMACD | IF_TYPE_IEEE80211 | IF_TYPE_WWANPP | IF_TYPE_WWANPP2
    ) && adapter.hardware == Some(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv6Addr;
    use windows::Win32::NetworkManagement::IpHelper::IF_TYPE_PROP_VIRTUAL;
    use windows::Win32::NetworkManagement::Ndis::IfOperStatusDown;

    fn adapter(name: &str, luid: u64, if_type: u32, hardware: bool, ipv4_metric: u32, gateway: bool) -> NetworkAdapter {
        NetworkAdapter {
            index: 1,
            luid,
            friendly_name: name.to_owned(),
            description: String::new(),
            if_type,
            oper_status: IfOperStatusUp.0,
            hardware: Some(hardware),
            mtu: 1500,
            ipv4_metric,
            ipv4_address: Some(Ipv4Addr::new(10, 0, 0, 2)),
            gateways: if gateway { vec![Ipv4Addr::new(10, 0, 0, 1).into()] } else { Vec::new() },
        }
    }

    fn selected_name(adapters: &[NetworkAdapter]) -> Option<&str> {
        select_adapter(adapters).map(|(adapter, _)| adapter.friendly_name.as_str())
    }

    #[test]
    fn prefers_hardware_over_lower_metric_virtual() {
        let adapters = [
            adapter("vEthernet (WSL)", 1, IF_TYPE_ETHERNET_CSMACD, false, 5, true),
            adapter("Wi-Fi", 2, IF_TYPE_IEEE80211, true, 50, true),
        ];
        assert_eq!(selected_name(&adapters), Some("Wi-Fi"));
    }

    #[test]
    fn falls_back_to_lowest_metric_default_route() {
        let adapters = [
            adapter("Ethernet", 1, IF_TYPE_ETHERNET_CSMACD, true, 25, false),
            adapter("vEthernet (WSL)", 2, IF_TYPE_ETHERNET_CSMACD, false, 25, true),
            adapter("Other VPN", 3, IF_TYPE_PROP_VIRTUAL, false, 5, true),
        ];
        assert_eq!(selected_name(&adapters), Some("Other VPN"));
    }

    #[test]
    fn requires_up_gateway_and_ipv4() {
        let mut down = adapter("Wi-Fi", 1, IF_TYPE_IEEE80211, true, 50, true);
        down.oper_status = IfOperStatusDown.0;
        let mut ipv6_gateway_only = adapter("Ethernet", 2, IF_TYPE_ETHERNET_CSMACD, true, 25, false);
        ipv6_gateway_only.gateways = vec![Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1).into()];
        let no_gateway = adapter("Ethernet 2", 3, IF_TYPE_ETHERNET_CSMACD, true, 25, false);
        assert_eq!(selected_name(&[down, ipv6_gateway_only, no_gateway]), None);
    }
}

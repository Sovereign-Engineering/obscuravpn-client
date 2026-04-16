use obscuravpn_client::net::NetworkInterface;
use obscuravpn_client::positive_u31::PositiveU31;
use std::net::{IpAddr, Ipv4Addr};
use std::time::Duration;
use tokio::sync::watch::{Receiver, Sender, channel};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Foundation::{ERROR_IO_PENDING, HANDLE};
use windows::Win32::NetworkManagement::IpHelper::{
    CancelIPChangeNotify, GetIfEntry2, IF_TYPE_ETHERNET_CSMACD, IF_TYPE_IEEE80211, MIB_IF_ROW2, NotifyAddrChange,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::Networking::WinSock::{AF_INET, SOCKADDR_IN};
use windows::Win32::System::IO::OVERLAPPED;
use windows::Win32::System::Threading::{CreateEventW, INFINITE, ResetEvent, WaitForSingleObject};

use crate::service::os::windows::gaa::GAABufferInit;

const WATCH_ERROR_BACKOFF: Duration = Duration::from_secs(1);

pub fn watch_active_adapter() -> Receiver<Option<NetworkInterface>> {
    let (sender, receiver) = channel(None);
    std::thread::spawn(move || watch_active_adapter_thread(&sender));
    receiver
}

fn watch_active_adapter_thread(sender: &Sender<Option<NetworkInterface>>) {
    loop {
        watch_addr_changes(sender);
        std::thread::sleep(WATCH_ERROR_BACKOFF);
    }
}

/// Creates an event handle, watches for address changes, and cleans up. Returns when an error occurs.
fn watch_addr_changes(sender: &Sender<Option<NetworkInterface>>) {
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
        let Ok(adapter) = get_active_physical_adapter() else {
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

    if let Err(error) = unsafe { CloseHandle(event_handle) } {
        tracing::warn!(message_id = "oLmLePMW", ?error, "failed to close event handle");
    }
}

pub struct PhysicalAdapter {
    name: String,
    index: u32,
    ip: IpAddr,
    pub mtu: u32,
}

/// Walk the adapter list returned by `GetAdaptersAddresses` and return the
/// first active, hardware, ethernet/wifi adapter that has a default gateway
/// and an IPv4 address.
pub fn find_active_physical_adapter() -> Result<Option<PhysicalAdapter>, ()> {
    let Some(gaa) = GAABufferInit::new()? else {
        return Ok(None);
    };

    let mut current = gaa.first;

    while !current.is_null() {
        // SAFETY: `current` points into pre-allocated `buffer`
        // The buffer was populated by `GetAdaptersAddresses` which guarantees a
        // valid linked list of `IP_ADAPTER_ADDRESSES_LH` structs.
        let adapter = unsafe { &*current };

        let if_type = adapter.IfType;
        // ethernet or wifi
        let is_media_type_ok = if_type == IF_TYPE_ETHERNET_CSMACD || if_type == IF_TYPE_IEEE80211;
        let is_up = adapter.OperStatus == IfOperStatusUp;

        let mut is_hardware = false;
        if is_media_type_ok {
            // SAFETY: Although these are union layers, it is a Microsoft pattern for memory alignment and "bulk copying."
            // Rather than "one or the other" field being defined, the intended field (e.g. IfIndex) is always defined and the
            // "Alignment" field exists to align the structure as well as for copying data at once.
            // https://learn.microsoft.com/windows/win32/api/iptypes/ns-iptypes-ip_adapter_addresses_lh
            let mut row = MIB_IF_ROW2 { InterfaceIndex: unsafe { adapter.Anonymous1.Anonymous.IfIndex }, ..Default::default() };
            // SAFETY: `row.InterfaceIndex` was set above; `GetIfEntry2` populates the
            // remaining fields of the struct.
            let res = unsafe { GetIfEntry2(&mut row) };
            // https://learn.microsoft.com/windows/win32/api/netioapi/ns-netioapi-mib_if_row2
            is_hardware = res.0 == 0 && (row.InterfaceAndOperStatusFlags._bitfield & 1/* HardwareInterface */) != 0;
        }

        if is_media_type_ok && is_hardware && is_up && !adapter.FirstGatewayAddress.is_null() {
            let mut unicast = adapter.FirstUnicastAddress;

            while !unicast.is_null() {
                // SAFETY: `unicast` is non-null (checked above) and points into the adapter
                // linked list populated by `GetAdaptersAddresses`.
                let ua = unsafe { &*unicast };
                // SAFETY: `lpSockaddr` is a valid pointer set by `GetAdaptersAddresses` for
                // each unicast address entry.
                let sockaddr = unsafe { &*ua.Address.lpSockaddr };

                if sockaddr.sa_family == AF_INET {
                    // SAFETY: `FriendlyName` is a valid null-terminated wide string set by
                    // `GetAdaptersAddresses`.
                    let name = match unsafe { adapter.FriendlyName.to_string() } {
                        Ok(name) => name,
                        Err(error) => {
                            tracing::error!(message_id = "cNWuYmcV", ?error, "failed to read adapter friendly name");
                            return Err(());
                        }
                    };
                    // SAFETY: Union access — see the SAFETY comment on the identical access above.
                    let index = unsafe { adapter.Anonymous1.Anonymous.IfIndex };
                    // SAFETY: We verified `sa_family == AF_INET`, so the sockaddr can be casted to a `SOCKADDR_IN`.
                    // The pointer is valid for the lifetime of `buffer`.
                    // Read more: https://learn.microsoft.com/windows/win32/api/ws2def/ns-ws2def-socket_address
                    let sa_in = unsafe { &*(ua.Address.lpSockaddr as *const SOCKADDR_IN) };
                    // SAFETY: Union access — `S_addr` is the raw u32 representation of the IPv4
                    // address, which is always valid to read when the family is `AF_INET`.
                    let ipv4 = Ipv4Addr::from_bits(u32::from_be(unsafe { sa_in.sin_addr.S_un.S_addr }));
                    return Ok(Some(PhysicalAdapter { name, index, ip: IpAddr::V4(ipv4), mtu: adapter.Mtu }));
                }

                unicast = ua.Next;
            }
        }

        current = adapter.Next;
    }
    Ok(None)
}

fn get_active_physical_adapter() -> Result<Option<NetworkInterface>, ()> {
    let Some(adapter) = find_active_physical_adapter()? else {
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
    Ok(Some(NetworkInterface { name: adapter.name, index, ip: adapter.ip, mtu }))
}

#[test]
fn test_not_wsl_vethernet() {
    let physical_adapter = get_active_physical_adapter().unwrap().unwrap();
    assert!(!physical_adapter.name.contains("vEthernet"));
    println!("{physical_adapter:?}");
}

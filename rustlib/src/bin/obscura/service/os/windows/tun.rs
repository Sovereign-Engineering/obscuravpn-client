use bytes::Bytes;
use ipnetwork::Ipv6Network;
use obscuravpn_client::os::packet_buffer::PacketBuffer;
use obscuravpn_client::quicwg::QuicWgConnPacketSender;
use obscuravpn_client::rate_limited_log;
use obscuravpn_client::tokio::AbortOnDrop;
use ring::digest;
use std::io::Read;
use std::time::Duration;
use tokio::task::spawn_blocking;
use wintun::Wintun;

use crate::service::os::windows::WindowsServiceStartError;
use crate::service::os::windows::iphelper;
use crate::service::os::windows::iphelper::flush_dns_cache;
use crate::service::os::windows::nrpt;
use std::net::IpAddr;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

/// SHA-256 hash of the authentic wintun.dll, calculated at build time.
const WINTUN_DLL_SHA256: &str = env!("WINTUN_DLL_SHA256");

const TUN_MIN_LOG_SILENCE: Duration = Duration::from_secs(5);

// If the adapter name contains spaces, setting the wintun adapter address does not work
//  due to an escape bug when calling netsh
const TUN_NAME: &str = "ObscuraVPN";

pub struct Tun {
    adapter: Arc<wintun::Adapter>,
    session: Arc<wintun::Session>,
    read_task: Mutex<Option<AbortOnDrop>>,
}

impl Tun {
    pub async fn create() -> Result<Self, WindowsServiceStartError> {
        // cleanup DNS routing in case of unclean disconnect
        if let Ok(true) = nrpt::delete_rules() {
            let _ = flush_dns_cache().await;
        }
        // SECURITY: To protect against privilege escalation when loading the relative wintun.dll,
        // verify that the file matches the pre-calculated SHA-256 hash from build time before loading.
        let wintun = verify_and_load_wintun()?;
        let adapter = match wintun::Adapter::open(&wintun, TUN_NAME) {
            Ok(a) => a,
            Err(error) => {
                tracing::warn!(message_id = "5ImYKHdv", ?error, "could not load wintun adapter, will try to create one");
                // If loading fails (e.g. doesn't exist), create one
                // THIS REQUIRES Administrator privileges
                // The GUID can be hard coded and provided
                wintun::Adapter::create(&wintun, TUN_NAME, "QUICWG", None).map_err(WindowsServiceStartError::CreateWintunAdapter)?
            }
        };
        let session = adapter
            .start_session(wintun::MAX_RING_CAPACITY)
            .map_err(WindowsServiceStartError::StartWintunSession)?;
        Ok(Tun { adapter, session: session.into(), read_task: Mutex::new(None) })
    }

    pub fn send(&self, packet: Bytes) {
        match u16::try_from(packet.len()) {
            Ok(packet_size) => {
                let packet_res = self.session.allocate_send_packet(packet_size);
                match packet_res {
                    Ok(mut packet_to_send) => {
                        let bytes: &mut [u8] = packet_to_send.bytes_mut();
                        bytes.copy_from_slice(&packet);
                        self.session.send_packet(packet_to_send);
                    }
                    Err(error) => {
                        rate_limited_log!(
                            TUN_MIN_LOG_SILENCE,
                            tracing::error!(message_id = "s1G0fKYL", ?error, "could not allocate packet on wintun adapter")
                        );
                    }
                }
            }
            Err(_) => {
                rate_limited_log!(
                    TUN_MIN_LOG_SILENCE,
                    tracing::error!(
                        message_id = "C3y8mGrT",
                        packet_size = packet.len(),
                        "cannot send packet: size exceeds u16::MAX"
                    )
                );
            }
        }
    }

    fn put_packet_in_buffer(packet: wintun::Packet, buffer: &mut [u8]) -> Result<u16, ()> {
        let packet_bytes = packet.bytes();
        match u16::try_from(packet_bytes.len()) {
            Ok(n) => {
                // Check if packet fits in the available buffer space
                if packet_bytes.len() <= buffer.len() {
                    buffer[..packet_bytes.len()].copy_from_slice(packet_bytes);
                    return Ok(n);
                } else {
                    tracing::error!(
                        message_id = "B2x9kFpQ",
                        packet_size = packet_bytes.len(),
                        buffer_size = buffer.len(),
                        "packet too large for available buffer space, dropping packet"
                    );
                }
            }
            Err(_) => {
                tracing::error!(
                    message_id = "A1s4jdil",
                    packet_size = packet_bytes.len(),
                    "ignoring oversized packet from tun device (exceeds u16::MAX)"
                );
            }
        }
        Err(())
    }

    async fn receive(session: &Arc<wintun::Session>, packet_buffer: &mut PacketBuffer) {
        if let Some(buffer) = packet_buffer.buffer() {
            let packet = loop {
                let session = session.clone();
                match spawn_blocking(move || session.receive_blocking()).await {
                    Ok(Ok(packet)) => break packet,
                    Ok(Err(error)) => {
                        rate_limited_log!(
                            TUN_MIN_LOG_SILENCE,
                            tracing::error!(message_id = "UD939201", ?error, "failed to receive from wintun")
                        );
                    }
                    Err(error) => {
                        rate_limited_log!(
                            TUN_MIN_LOG_SILENCE,
                            tracing::error!(message_id = "qlEOseOs", ?error, "failed to join wintun receive")
                        );
                    }
                }
            };
            if let Ok(n) = Self::put_packet_in_buffer(packet, buffer) {
                packet_buffer.commit(n);
            }
        }

        while let Some(buffer) = packet_buffer.buffer() {
            let result = session.try_receive();
            match result {
                Ok(None) => return,
                Ok(Some(packet)) => {
                    if let Ok(n) = Self::put_packet_in_buffer(packet, buffer) {
                        packet_buffer.commit(n);
                    }
                }
                Err(error) => {
                    rate_limited_log!(
                        TUN_MIN_LOG_SILENCE,
                        tracing::error!(message_id = "WXll4YJu", ?error, "failed to receive from wintun")
                    );
                }
            }
        }
    }

    pub async fn set_config(&self, mtu: u16, ipv4: Ipv4Addr, ipv6: Ipv6Network, dns: Option<Vec<IpAddr>>) -> Result<(), ()> {
        // Attempt all config steps regardless of individual failures to minimize leaks until intentionally disconnecting.
        // E.g. DNS queries shouldn't leak because route setup failed.
        let mut result = Ok(());
        result = result
            .and(iphelper::set_mtu(&self.adapter, mtu).await)
            .and(iphelper::set_ipv4_address(&self.adapter, ipv4).await)
            .and(iphelper::set_ipv6_address(&self.adapter, ipv6).await)
            .and(iphelper::set_dns_servers(&self.adapter, dns.as_deref().unwrap_or_default()).await)
            .and(iphelper::set_low_metric(&self.adapter))
            .and(iphelper::add_routes(&self.adapter));

        // Avoid DNS outage by redirecting after adding routes
        if let Some(dns) = dns {
            result = result.and(nrpt::create_rule(&dns).or_else(|_| nrpt::delete_rules().map(drop)));
        } else {
            result = result.and(nrpt::delete_rules().map(drop));
        }
        result = result.and(flush_dns_cache().await);

        result
    }

    pub fn spawn_read_task(&self, tunnel: QuicWgConnPacketSender) {
        let mut read_task = self.read_task.lock().unwrap();
        let session = self.session.clone();
        *read_task = Some(AbortOnDrop::spawn(async move {
            let mut packet_buffer = PacketBuffer::default();
            loop {
                Self::receive(&session, &mut packet_buffer).await;
                tunnel.send(packet_buffer.take_iter());
            }
        }));
    }

    pub async fn shutdown(&self) -> Result<(), ()> {
        let mut result = Ok(());
        result = result.and(nrpt::delete_rules().map(drop));
        result = result.and(flush_dns_cache().await);
        result = result.and(iphelper::reset_interface_metric(&self.adapter));
        result = result.and(iphelper::remove_routes(&self.adapter));
        result
    }
}

/// SECURITY: Verify that the wintun.dll at `dll_path` matches the SHA-256 hash calculated at
/// build time. This should prevent using a tampered or replaced DLL, which could lead to privilege
/// escalation since the service runs with elevated privileges.
fn verify_and_load_wintun() -> Result<Wintun, WindowsServiceStartError> {
    let dll_path = std::env::current_exe()
        .map_err(WindowsServiceStartError::CurrentExePath)?
        .with_file_name("wintun.dll");

    // SECURITY: Keep the file handle open through hash verification and DLL loading to prevent
    // TOCTOU attacks where the DLL could be replaced between verification and loading.
    // On Windows, an open file handle prevents the file from being written to or deleted.
    let _dll_file = std::fs::File::open(&dll_path).map_err(WindowsServiceStartError::WintunDllRead)?;
    let mut dll_bytes = Vec::new();
    (&_dll_file)
        .read_to_end(&mut dll_bytes)
        .map_err(WindowsServiceStartError::WintunDllRead)?;

    let actual_hash = digest::digest(&digest::SHA256, &dll_bytes);
    let actual_hex: String = actual_hash.as_ref().iter().map(|b| format!("{b:02x}")).collect();

    (&actual_hex == WINTUN_DLL_SHA256)
        .then_some(())
        .ok_or_else(|| WindowsServiceStartError::WintunDllHashMismatch {
            dll_path: dll_path.clone(),
            expected: WINTUN_DLL_SHA256.to_string(),
            actual: actual_hex,
        })?;

    tracing::info!(message_id = "mtxTbQFv", "wintun.dll hash verified successfully");
    // `_dll_file` is still open here, preventing the DLL from being replaced before loading.
    unsafe { wintun::load_from_path(&dll_path) }.map_err(WindowsServiceStartError::LoadWintunDll)
    // `_dll_file` is dropped here, releasing the read lock on the DLL.
}

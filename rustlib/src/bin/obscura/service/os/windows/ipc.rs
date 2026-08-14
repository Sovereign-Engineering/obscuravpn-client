use super::start_error::WindowsServiceStartError;
use crate::service::os::MAX_IPC_MESSAGE_LEN;
use flume::{Receiver, Sender, bounded};
use obscuravpn_client::int_helper::u32_into_usize;
use std::ffi::c_void;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tokio::time::timeout;
use windows::Win32::Security::SECURITY_ATTRIBUTES;

use super::PACKAGE_FAMILY_NAME;
use obscuravpn_client::os::windows::sddl::{DACL, FileRights, Inherit, SA_LENGTH, SecurityDescriptor, Trustee};

pub const PIPE_NAME: &str = r"\\.\pipe\obscuravpn";
/// Drop a connected client that doesn't send a full message within this window.
/// Local IPC reads should complete in milliseconds; this only fires on a stalled
/// or hostile peer that opened the pipe to occupy an instance.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

pub struct ServiceIpc {
    receiver: Receiver<(Vec<u8>, Box<dyn FnOnce(Vec<u8>) + Send>)>,
}

impl ServiceIpc {
    pub fn new() -> Result<Self, WindowsServiceStartError> {
        let security_attrs = PipeSecurityAttributes::new().map_err(|error| {
            tracing::error!(message_id = "aB1cD2eF", ?error, "failed to build pipe security attributes");
            WindowsServiceStartError::CreateNamedPipe(error)
        })?;

        let server = unsafe {
            ServerOptions::new()
                .first_pipe_instance(true)
                .create_with_security_attributes_raw(PIPE_NAME, (&raw const *security_attrs.sa).cast_mut().cast::<c_void>())
        }
        .map_err(|error| {
            tracing::error!(message_id = "v0jjUdAJ", ?error, "failed to create named pipe");
            WindowsServiceStartError::CreateNamedPipe(error)
        })?;

        // ensure that `Self::next()` is cancel safe by decoupling it from the incremental progress on pipe streams.
        let (sender, receiver) = bounded::<(Vec<u8>, Box<dyn FnOnce(Vec<u8>) + Send>)>(0);

        tokio::spawn(async move {
            Self::accept_loop(server, sender, security_attrs).await;
        });

        Ok(Self { receiver })
    }

    async fn accept_loop(
        mut server: NamedPipeServer,
        sender: Sender<(Vec<u8>, Box<dyn FnOnce(Vec<u8>) + Send>)>,
        security_attrs: PipeSecurityAttributes,
    ) {
        while !sender.is_disconnected() {
            if let Err(error) = server.connect().await {
                tracing::error!(message_id = "ODKeDHzZ", ?error, "named pipe connect failed");
                panic!("named pipe accept errors are not recoverable: {error}");
            }

            let connected_client = server;
            server = match unsafe {
                ServerOptions::new().create_with_security_attributes_raw(PIPE_NAME, (&raw const *security_attrs.sa).cast_mut().cast::<c_void>())
            } {
                Ok(s) => s,
                Err(error) => {
                    tracing::error!(message_id = "XDc5xmTV", ?error, "failed to create next named pipe instance: {error}");
                    panic!("failed to create named pipe instance: {error}");
                }
            };

            let sender = sender.clone();
            tokio::spawn(async move {
                let _: Result<(), ()> = Self::handle_connection(connected_client, sender).await;
            });
        }
        tracing::info!(message_id = "OA2Rkelm", "stop listening for named pipe connections");
    }

    pub async fn next(&self) -> (Vec<u8>, Box<dyn FnOnce(Vec<u8>) + Send>) {
        self.receiver.recv_async().await.expect("API pipe recv failed")
    }

    async fn handle_connection(mut pipe: NamedPipeServer, sender: Sender<(Vec<u8>, Box<dyn FnOnce(Vec<u8>) + Send>)>) -> Result<(), ()> {
        tracing::info!(message_id = "pj6ESzQ1", "handling new named pipe connection");

        let mut len = [0u8; 4];
        timeout(READ_TIMEOUT, pipe.read_exact(&mut len))
            .await
            .map_err(|_elapsed| {
                tracing::warn!(message_id = "40I5IPW2", ?READ_TIMEOUT, "timed out reading message length from named pipe");
            })?
            .map_err(|error| {
                tracing::error!(message_id = "Awz3nfz0", ?error, "failed to read message length from named pipe: {error}");
            })?;
        let len = u32::from_be_bytes(len);
        if len > MAX_IPC_MESSAGE_LEN {
            tracing::error!(message_id = "QPw0P7zV", len, "message on named pipe too long");
            return Err(());
        }
        let mut message: Vec<u8> = vec![0; u32_into_usize(len)];
        timeout(READ_TIMEOUT, pipe.read_exact(&mut message))
            .await
            .map_err(|_elapsed| {
                tracing::warn!(message_id = "l7SPBC2z", ?READ_TIMEOUT, "timed out reading message body from named pipe");
            })?
            .map_err(|error| {
                tracing::error!(message_id = "BgJTZvYg", ?error, "failed to read message from named pipe: {error}");
            })?;

        let response_fn = move |response: Vec<u8>| {
            tokio::spawn(async move {
                let len = u32::try_from(response.len())
                    .map_err(|error| {
                        tracing::error!(message_id = "pV7wKd3B", ?error, "response too long for length prefix");
                    })?
                    .to_be_bytes();
                pipe.write_all(&len).await.map_err(|error| {
                    tracing::error!(message_id = "hECmTcej", ?error, "failed to write response length to named pipe: {error}");
                })?;
                pipe.write_all(&response).await.map_err(|error| {
                    tracing::error!(message_id = "hlLf1Thk", ?error, "failed to write response to named pipe: {error}");
                })?;
                pipe.flush().await.map_err(|error| {
                    tracing::error!(message_id = "nPipeFlsh", ?error, "failed to flush named pipe: {error}");
                })?;
                Result::<(), ()>::Ok(())
            });
        };

        _ = sender.send_async((message, Box::new(response_fn))).await;
        tracing::info!(message_id = "nPipeDone", "finished handling named pipe connection");
        Ok(())
    }
}

/// Owns the `SECURITY_DESCRIPTOR` referenced by a Windows `SECURITY_ATTRIBUTES`. The DACL grants
/// full access to LocalSystem and Administrators, and read/write access to the Obscura VPN package family name.
struct PipeSecurityAttributes {
    /// Backing security descriptor referenced by `sa.lpSecurityDescriptor`; kept alive for `Drop`.
    _sd: SecurityDescriptor,
    /// Heap-allocated `SECURITY_ATTRIBUTES` whose address is exposed as a raw pointer.
    sa: Box<SECURITY_ATTRIBUTES>,
}

// SAFETY: Every raw pointer inside this struct points into memory that the struct owns exclusively.
// After construction the memory is only read (never mutated), so sharing across threads is safe.
unsafe impl Send for PipeSecurityAttributes {}
unsafe impl Sync for PipeSecurityAttributes {}

impl PipeSecurityAttributes {
    fn new() -> std::io::Result<Self> {
        let dacl = DACL::new()
            .allow(FileRights::FullAccess, Trustee::local_system(), Inherit::None)
            .allow(FileRights::FullAccess, Trustee::builtin_administrators(), Inherit::None)
            // Restricted: pin read/write to our packaged GUI via a conditional WIN://SYSAPPID ACE.
            .allow_packaged(FileRights::ReadWrite, PACKAGE_FAMILY_NAME);

        let sd = dacl.build()?;

        // `SECURITY_ATTRIBUTES` is three POD fields. Zero-initializing leaves
        // `bInheritHandle == FALSE` (the default we want); `nLength` and
        // `lpSecurityDescriptor` are overwritten below.
        // SAFETY: zero is a valid bit pattern for `SECURITY_ATTRIBUTES`.
        let mut sa: Box<SECURITY_ATTRIBUTES> = Box::new(unsafe { std::mem::zeroed() });
        sa.nLength = SA_LENGTH;
        sa.lpSecurityDescriptor = sd.as_ptr();

        Ok(Self { _sd: sd, sa })
    }
}

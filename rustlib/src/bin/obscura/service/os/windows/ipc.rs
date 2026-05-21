use super::start_error::WindowsServiceStartError;
use crate::service::os::MAX_IPC_MESSAGE_LEN;
use flume::{Receiver, Sender, bounded};
use std::ffi::c_void;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};
use tokio::time::timeout;
use windows::Win32::Security::{
    ACCESS_ALLOWED_ACE, ACL, ACL_REVISION, AddAccessAllowedAce, AllocateAndInitializeSid, FreeSid, GetLengthSid, InitializeAcl,
    InitializeSecurityDescriptor, PSECURITY_DESCRIPTOR, PSID, SECURITY_ATTRIBUTES, SECURITY_DESCRIPTOR, SECURITY_NT_AUTHORITY,
    SetSecurityDescriptorDacl,
};

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
                .create_with_security_attributes_raw(PIPE_NAME, security_attrs.sa.as_ref() as *const SECURITY_ATTRIBUTES as *mut c_void)
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
                ServerOptions::new()
                    .create_with_security_attributes_raw(PIPE_NAME, security_attrs.sa.as_ref() as *const SECURITY_ATTRIBUTES as *mut c_void)
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
        let mut message: Vec<u8> = vec![0; len as usize];
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
                let len = (response.len() as u32).to_be_bytes();
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

/// RAII wrapper around a `PSID` returned by `AllocateAndInitializeSid`.
/// Calls `FreeSid` on drop.
struct OwnedSid(PSID);

impl OwnedSid {
    /// Allocate a SID for the given identifier authority and single sub-authority.
    fn new(authority: &mut windows::Win32::Security::SID_IDENTIFIER_AUTHORITY, sub_authority: u32) -> std::io::Result<Self> {
        let mut sid = PSID::default();
        // SAFETY: `authority` and `sid` point to valid, properly aligned storage
        // that lives for the duration of this call.
        unsafe { AllocateAndInitializeSid(authority, 1, sub_authority, 0, 0, 0, 0, 0, 0, 0, &mut sid) }?;
        Ok(Self(sid))
    }

    fn as_psid(&self) -> PSID {
        self.0
    }
}

impl Drop for OwnedSid {
    fn drop(&mut self) {
        // SAFETY: `self.0` was returned by `AllocateAndInitializeSid` and
        // has not been freed yet.
        unsafe { FreeSid(self.0) };
    }
}

/// Owns all heap memory referenced by a Windows `SECURITY_ATTRIBUTES` that
/// grants `NT AUTHORITY\Authenticated Users` `GENERIC_READ | GENERIC_WRITE`
/// access.
struct PipeSecurityAttributes {
    /// SID referenced by the ACE inside `_acl_buf`; kept alive for its `Drop`.
    _sid: OwnedSid,
    /// Heap buffer holding the binary ACL; referenced by `_sd`.
    _acl_buf: Box<[u8]>,
    /// Heap-allocated `SECURITY_DESCRIPTOR`; referenced by `sa`.
    _sd: Box<SECURITY_DESCRIPTOR>,
    /// Heap-allocated `SECURITY_ATTRIBUTES` whose address is exposed as a raw pointer.
    sa: Box<SECURITY_ATTRIBUTES>,
}

// SAFETY: Every raw pointer inside this struct points into heap memory that
// the struct owns exclusively.  After construction the memory is only read
// (never mutated), so sharing across threads is safe.
unsafe impl Send for PipeSecurityAttributes {}
unsafe impl Sync for PipeSecurityAttributes {}

impl PipeSecurityAttributes {
    fn new() -> std::io::Result<Self> {
        // S-1-5-11  NT AUTHORITY\Authenticated Users
        const SECURITY_AUTHENTICATED_USER_RID: u32 = 0x0B;
        const SECURITY_DESCRIPTOR_REVISION1: u32 = 1;
        // GENERIC_READ | GENERIC_WRITE
        const ACCESS_MASK: u32 = 0x8000_0000 | 0x4000_0000;

        let mut nt_authority = SECURITY_NT_AUTHORITY;
        let sid = OwnedSid::new(&mut nt_authority, SECURITY_AUTHENTICATED_USER_RID)?;

        // SAFETY: `sid` is a live SID returned by `AllocateAndInitializeSid`.
        let sid_len = unsafe { GetLengthSid(sid.as_psid()) } as usize;
        // ACL header + ACCESS_ALLOWED_ACE (SidStart placeholder replaced
        // by the full variable-length SID).
        let acl_size = std::mem::size_of::<ACL>() + std::mem::size_of::<ACCESS_ALLOWED_ACE>() - std::mem::size_of::<u32>() + sid_len;
        let mut acl_buf: Box<[u8]> = vec![0u8; acl_size].into_boxed_slice();
        let p_acl = acl_buf.as_mut_ptr() as *mut ACL;

        // SAFETY: `p_acl` points to `acl_size` writable, properly aligned bytes
        // owned by `acl_buf`; `acl_size` is the buffer's true length.
        unsafe { InitializeAcl(p_acl, acl_size as u32, ACL_REVISION) }?;
        // SAFETY: `p_acl` is a freshly initialized ACL with capacity for this
        // ACE; `sid` is live for the duration of the call.
        unsafe { AddAccessAllowedAce(p_acl, ACL_REVISION, ACCESS_MASK, sid.as_psid()) }?;

        // `InitializeSecurityDescriptor` writes the descriptor in place, so the
        // storage must already be initialized before we form a pointer to it.
        // All-zero is a valid bit pattern for `SECURITY_DESCRIPTOR` (every field
        // is an integer or pointer for which zero is a legal value), so
        // `mem::zeroed` produces a sound initial state without enumerating
        // every field — the Windows API then overwrites them.
        // SAFETY: zero is a valid bit pattern for `SECURITY_DESCRIPTOR`.
        let mut sd: Box<SECURITY_DESCRIPTOR> = Box::new(unsafe { std::mem::zeroed() });
        // `sd` is heap-allocated, so `sd.as_mut()` gives a stable address
        // that remains valid after the Box is moved into the struct.
        let p_sd = PSECURITY_DESCRIPTOR(sd.as_mut() as *mut _ as *mut c_void);
        // SAFETY: `p_sd` points to writable, properly aligned storage for one
        // `SECURITY_DESCRIPTOR`.
        unsafe { InitializeSecurityDescriptor(p_sd, SECURITY_DESCRIPTOR_REVISION1) }?;
        // SAFETY: `p_sd` has been initialized by the call above; `p_acl` points
        // to a valid ACL that outlives the descriptor (both are owned by `Self`).
        unsafe { SetSecurityDescriptorDacl(p_sd, true, Some(p_acl), false) }?;

        // `SECURITY_ATTRIBUTES` is three POD fields. Zero-initializing leaves
        // `bInheritHandle == FALSE` (the default we want); `nLength` and
        // `lpSecurityDescriptor` are overwritten below.
        // SAFETY: zero is a valid bit pattern for `SECURITY_ATTRIBUTES`.
        let mut sa: Box<SECURITY_ATTRIBUTES> = Box::new(unsafe { std::mem::zeroed() });
        sa.nLength = std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32;
        sa.lpSecurityDescriptor = sd.as_mut() as *mut _ as *mut c_void;

        Ok(Self { _sid: sid, _acl_buf: acl_buf, _sd: sd, sa })
    }
}

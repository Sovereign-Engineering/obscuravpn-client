use crate::service::os::MAX_IPC_MESSAGE_LEN;
use crate::service::os::linux::service_lock::ServiceLock;
use crate::service::os::linux::start_error::LinuxServiceStartError;
use flume::{Receiver, Sender, bounded};
use nix::sys::stat::Mode;
use nix::unistd::{Gid, Uid, User, getgrouplist};
use obscuravpn_client::int_helper::u32_into_usize;
use obscuravpn_client::linux::ipc::{LIVE_GROUPS_SOCKET_PATH, LinuxIpcHeader, SOCKET_PATH};
use obscuravpn_client::manager_cmd::PeerUid;
use obscuravpn_client::version::release_version;
use std::ffi::CString;
use std::fs;
use std::io::ErrorKind;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::oneshot;

pub struct LinuxIpcRequest {
    pub message: Vec<u8>,
    pub peer_uid: PeerUid,
    response_tx: oneshot::Sender<Vec<u8>>,
}

impl LinuxIpcRequest {
    fn new(message: Vec<u8>, peer_uid: PeerUid) -> (Self, oneshot::Receiver<Vec<u8>>) {
        let (response_tx, response_rx) = oneshot::channel();
        (Self { message, peer_uid, response_tx }, response_rx)
    }

    pub fn respond(self, json_response: Vec<u8>) {
        let _ = self.response_tx.send(json_response);
    }
}

pub struct ServiceIpc {
    receiver: Receiver<LinuxIpcRequest>,
}

impl ServiceIpc {
    pub async fn new(_lock: &ServiceLock) -> Result<Self, LinuxServiceStartError> {
        let socket = Self::bind_listener(SOCKET_PATH, None)?;
        let live_groups_socket = Self::bind_listener(LIVE_GROUPS_SOCKET_PATH, Some(Mode::from_bits_truncate(0o666)))?;

        // ensure that `Self::next()` is cancel safe by decoupling it from the incremental progress on socket streams.
        let (sender, receiver) = bounded::<LinuxIpcRequest>(0);
        Self::spawn_accept_loop(socket, sender.clone(), false);
        Self::spawn_accept_loop(live_groups_socket, sender, true);
        Ok(Self { receiver })
    }

    fn bind_listener(path: &str, mode: Option<Mode>) -> Result<UnixListener, LinuxServiceStartError> {
        fs::remove_file(path).or_else(|error| match error.kind() {
            ErrorKind::NotFound => Ok(()),
            kind => {
                tracing::error!(message_id = "GTtsZsdU", ?error, path, "failed to remove stale socket file: {error}");
                Err(match kind {
                    ErrorKind::PermissionDenied => LinuxServiceStartError::InsufficientPermissions,
                    _ => anyhow::Error::new(error).context("failed to remove stale socket file").into(),
                })
            }
        })?;

        let listener = UnixListener::bind(path).map_err(|error| {
            tracing::error!(message_id = "1WXBW1gj", ?error, path, "failed to bind socket: {error}");
            match error.kind() {
                ErrorKind::PermissionDenied => LinuxServiceStartError::InsufficientPermissions,
                _ => anyhow::Error::new(error).context("failed to create IPC socket").into(),
            }
        })?;
        if let Some(mode) = mode {
            fs::set_permissions(path, fs::Permissions::from_mode(mode.bits())).map_err(|error| {
                tracing::error!(message_id = "fW2nJq8X", ?error, path, "failed to set socket mode: {error}");
                LinuxServiceStartError::from(anyhow::Error::new(error).context("failed to set socket mode"))
            })?;
        }
        Ok(listener)
    }

    fn spawn_accept_loop(socket: UnixListener, sender: Sender<LinuxIpcRequest>, require_fresh_group_membership: bool) {
        tokio::spawn(async move {
            while !sender.is_disconnected() {
                let stream = match socket.accept().await {
                    Ok((stream, _)) => stream,
                    Err(error) => {
                        tracing::error!(message_id = "Y3lClT6m", ?error, "socket accept failed: {error}");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        continue;
                    }
                };
                let sender = sender.clone();
                tokio::spawn(async move {
                    let _: Result<(), ()> = Self::handle_stream(stream, sender, require_fresh_group_membership).await;
                });
            }
            tracing::info!(message_id = "dYp5Tr25", "stop listening for IPC connections");
        });
    }

    pub async fn next(&self) -> LinuxIpcRequest {
        self.receiver.recv_async().await.expect("uds task death is not recoverable")
    }

    async fn handle_stream(mut stream: UnixStream, sender: Sender<LinuxIpcRequest>, require_fresh_group_membership: bool) -> Result<(), ()> {
        tracing::info!(message_id = "M0sAFoC7", "handling new socket stream");

        let peer_uid = stream.peer_cred().map(|cred| PeerUid(cred.uid())).map_err(|error| {
            tracing::error!(message_id = "zK5nQw7Y", ?error, "failed to read peer credentials: {error}");
        })?;
        let authorized_uid = if require_fresh_group_membership {
            may_access_with_fresh_groups(peer_uid).await.then_some(peer_uid)
        } else {
            Some(peer_uid)
        };

        let header = LinuxIpcHeader { version: release_version().to_owned(), denied: authorized_uid.is_none() };
        header.write(&mut stream).await.map_err(|error| {
            tracing::error!(message_id = "cV2mXk8T", ?error, "failed to write IPC header to socket stream: {error}");
        })?;
        let Some(peer_uid) = authorized_uid else {
            let _ = stream.shutdown().await;
            return Ok(());
        };

        let mut len = [0u8; 4];
        stream.read_exact(&mut len).await.map_err(|error| {
            tracing::error!(
                message_id = "hfdWDTcp",
                ?error,
                "failed to read message length from socket stream: {error}"
            );
        })?;
        let len = u32::from_be_bytes(len);
        if len > MAX_IPC_MESSAGE_LEN {
            tracing::error!(message_id = "k9XmPq2R", len, "message on socket stream too long");
            return Err(());
        }
        let mut message: Vec<u8> = vec![0; u32_into_usize(len)];
        stream.read_exact(&mut message).await.map_err(|error| {
            tracing::error!(message_id = "GFf8wiV3", ?error, "failed to read message from socket stream: {error}");
        })?;

        let (request, response_rx) = LinuxIpcRequest::new(message, peer_uid);
        _ = sender.send_async(request).await;
        tracing::info!(
            message_id = "lx2Z8pCr",
            "received full message from socket stream, waiting for response callback"
        );

        let mut eof = [0u8; 1];
        let response = tokio::select! {
            biased;
            response = response_rx => response.map_err(|_| {
                tracing::error!(message_id = "uF5xHd7M", "response callback dropped without being called");
            })?,
            read_result = stream.read(&mut eof) => {
                match read_result {
                    Ok(0) => tracing::error!(message_id = "pT7cJm2X", "client hung up while waiting for response, dropping socket stream"),
                    Ok(_) => tracing::error!(message_id = "sL4gVw9D", "client sent more bytes while waiting for response, dropping socket stream"),
                    Err(error) => tracing::error!(message_id = "eK8nRb3Q", ?error, "socket stream read failed while waiting for response: {error}"),
                }
                return Err(());
            }
        };
        stream.write_all(&response).await.map_err(|error| {
            tracing::error!(message_id = "XijfChPl", ?error, "failed to write response to socket stream: {error}");
        })?;
        stream.shutdown().await.map_err(|error| {
            tracing::error!(message_id = "RRCdeq0M", ?error, "failed to close socket write stream: {error}");
        })?;
        // Sockets closed for writing on both sides don't linger, even if there's unread data, so we need to wait for the client to signal it's done reading.
        let n = stream.read(&mut [0u8; 1]).await.map_err(|error| {
            tracing::error!(message_id = "g90YsnwQ", ?error, "failed to read clean EOF from socket stream: {error}");
        })?;
        if n == 0 {
            tracing::info!(message_id = "CiLg0uHK", "client closed socket stream as expected");
        } else {
            tracing::error!(message_id = "MldiAfVK", "client sent more bytes than announced on socket stream");
        }
        Ok(())
    }
}

async fn may_access_with_fresh_groups(peer_uid: PeerUid) -> bool {
    let required_group = Gid::effective();
    let is_member = tokio::task::spawn_blocking(move || {
        let user = match User::from_uid(Uid::from_raw(peer_uid.0)) {
            Ok(Some(user)) => user,
            Ok(None) => {
                tracing::warn!(message_id = "jC8kXv3W", uid = peer_uid.0, "peer uid has no user database entry");
                return false;
            }
            Err(error) => {
                tracing::error!(
                    message_id = "mF5tZq8B",
                    ?error,
                    uid = peer_uid.0,
                    "failed to resolve peer uid to user: {error}"
                );
                return false;
            }
        };
        let Ok(name) = CString::new(user.name) else {
            tracing::error!(message_id = "rL2vNs6D", uid = peer_uid.0, "peer user name contains a NUL byte");
            return false;
        };
        getgrouplist(&name, user.gid)
            .map(|groups| groups.contains(&required_group))
            .unwrap_or_else(|error| {
                tracing::error!(
                    message_id = "xT4hJw9M",
                    ?error,
                    uid = peer_uid.0,
                    "failed to list peer group memberships: {error}"
                );
                false
            })
    })
    .await
    .unwrap_or_else(|error| {
        tracing::error!(message_id = "vN7bTk2H", ?error, "group membership lookup task failed: {error}");
        false
    });
    if !is_member {
        tracing::warn!(
            message_id = "yG2wPd6S",
            peer_uid = peer_uid.0,
            required_gid = required_group.as_raw(),
            "denying live-groups IPC connection, peer is not a member of the service group"
        );
    }
    is_member
}

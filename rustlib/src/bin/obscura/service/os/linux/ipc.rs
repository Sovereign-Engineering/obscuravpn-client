use crate::service::os::MAX_IPC_MESSAGE_LEN;
use crate::service::os::linux::service_lock::ServiceLock;
use crate::service::os::linux::start_error::LinuxServiceStartError;
use flume::{Receiver, Sender, bounded};
use obscuravpn_client::int_helper::u32_into_usize;
use obscuravpn_client::linux::ipc::{LinuxIpcHeader, SOCKET_PATH};
use obscuravpn_client::version::release_version;
use std::fs;
use std::io::ErrorKind;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::oneshot;

pub struct LinuxIpcRequest {
    pub message: Vec<u8>,
    response_tx: oneshot::Sender<Vec<u8>>,
}

impl LinuxIpcRequest {
    fn new(message: Vec<u8>) -> (Self, oneshot::Receiver<Vec<u8>>) {
        let (response_tx, response_rx) = oneshot::channel();
        (Self { message, response_tx }, response_rx)
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
        fs::remove_file(SOCKET_PATH).or_else(|error| match error.kind() {
            ErrorKind::NotFound => Ok(()),
            kind => {
                tracing::error!(message_id = "GTtsZsdU", ?error, "failed to remove stale socket file: {error}");
                Err(match kind {
                    ErrorKind::PermissionDenied => LinuxServiceStartError::InsufficientPermissions,
                    _ => anyhow::Error::new(error).context("failed to remove stale socket file").into(),
                })
            }
        })?;

        let socket = UnixListener::bind(SOCKET_PATH).map_err(|error| {
            tracing::error!(message_id = "1WXBW1gj", ?error, "failed to bind socket: {error}");
            match error.kind() {
                ErrorKind::PermissionDenied => LinuxServiceStartError::InsufficientPermissions,
                _ => anyhow::Error::new(error).context("failed to create IPC socket").into(),
            }
        })?;
        // ensure that `Self::next()` is cancel safe by decoupling it from the incremental progress on socket streams.
        let (sender, receiver) = bounded::<LinuxIpcRequest>(0);
        tokio::spawn(async move {
            while !sender.is_disconnected() {
                let Ok((stream, _)) = socket.accept().await.map_err(|error| {
                    tracing::error!(message_id = "Y3lClT6m", ?error, "socket accept failed: {error}");
                    panic!("socket accept errors are not recoverable: {error}");
                });

                let sender = sender.clone();
                tokio::spawn(async move {
                    let _: Result<(), ()> = Self::handle_stream(stream, sender).await;
                });
            }
            tracing::info!(message_id = "dYp5Tr25", "stop listening for IPC connections");
        });
        Ok(Self { receiver })
    }

    pub async fn next(&self) -> LinuxIpcRequest {
        self.receiver.recv_async().await.expect("uds task death is not recoverable")
    }

    async fn handle_stream(mut stream: UnixStream, sender: Sender<LinuxIpcRequest>) -> Result<(), ()> {
        tracing::info!(message_id = "M0sAFoC7", "handling new socket stream");

        LinuxIpcHeader { version: release_version().to_owned() }
            .write(&mut stream)
            .await
            .map_err(|error| {
                tracing::error!(message_id = "cV2mXk8T", ?error, "failed to write IPC header to socket stream: {error}");
            })?;

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

        let (request, response_rx) = LinuxIpcRequest::new(message);
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

use crate::service::os::linux::service_lock::ServiceLock;
use crate::service::os::linux::start_error::ServiceStartError;
use flume::{Receiver, Sender, bounded};
use std::fs;
use std::io::ErrorKind;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

pub const SOCKET_PATH: &str = "/run/obscura.sock";

pub struct ServiceIpc {
    receiver: Receiver<(Vec<u8>, Box<dyn FnOnce(Vec<u8>) + Send>)>,
}

impl ServiceIpc {
    pub async fn new(_lock: &ServiceLock) -> Result<Self, ServiceStartError> {
        fs::remove_file(SOCKET_PATH).or_else(|error| match error.kind() {
            ErrorKind::NotFound => Ok(()),
            kind => {
                tracing::error!(message_id = "GTtsZsdU", ?error, "failed to remove stale socket file: {error}");
                Err(match kind {
                    ErrorKind::PermissionDenied => ServiceStartError::InsufficientPermissions,
                    _ => anyhow::Error::new(error).context("failed to remove stale socket file").into(),
                })
            }
        })?;

        let socket = UnixListener::bind(SOCKET_PATH).map_err(|error| {
            tracing::error!(message_id = "1WXBW1gj", ?error, "failed to bind socket: {error}");
            match error.kind() {
                ErrorKind::PermissionDenied => ServiceStartError::InsufficientPermissions,
                _ => anyhow::Error::new(error).context("failed to create IPC socket").into(),
            }
        })?;
        // ensure that `Self::next()` is cancel safe by decoupling it from the incremental progress on socket streams.
        let (sender, receiver) = bounded::<(Vec<u8>, Box<dyn FnOnce(Vec<u8>) + Send>)>(0);
        tokio::spawn(async move {
            while !sender.is_disconnected() {
                let Ok((stream, _)) = socket.accept().await.map_err(|error| {
                    tracing::error!(message_id = "Y3lClT6m", ?error, "socket accept failed: error");
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

    pub async fn next(&self) -> (Vec<u8>, Box<dyn FnOnce(Vec<u8>) + Send>) {
        self.receiver.recv_async().await.expect("uds task death is not recoverable")
    }

    async fn handle_stream(mut stream: UnixStream, sender: Sender<(Vec<u8>, Box<dyn FnOnce(Vec<u8>) + Send>)>) -> Result<(), ()> {
        tracing::info!(message_id = "M0sAFoC7", "handling new socket stream");

        // TODO: send a build identifier to allow the client to ensure it is the same binary (command protocol has no stability guarantees)

        let mut len = [0u8; 4];
        stream.read_exact(&mut len).await.map_err(|error| {
            tracing::error!(
                message_id = "hfdWDTcp",
                ?error,
                "failed to read message length from socket stream: {error}"
            );
        })?;
        let len = u32::from_be_bytes(len);
        if len > 1_000_000 {
            tracing::error!(message_id = "k9XmPq2R", len, "message on socket stream too long");
            return Err(());
        }
        let mut message: Vec<u8> = vec![0; len as usize];
        stream.read_exact(&mut message).await.map_err(|error| {
            tracing::error!(message_id = "GFf8wiV3", ?error, "failed to read message from socket stream: {error}");
        })?;
        let response_fn = move |response: Vec<u8>| {
            tokio::spawn(async move {
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
                    tracing::error!(message_id = "MldiAfVK", "client sent {n} more bytes than announced on socket stream");
                }
                Result::<(), ()>::Ok(())
            });
        };
        _ = sender.send_async((message, Box::new(response_fn))).await;
        tracing::info!(message_id = "lx2Z8pCr", "finished handling socket stream");
        Ok(())
    }
}

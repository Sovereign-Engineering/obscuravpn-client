use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use crate::int_helper::u32_into_usize;
use crate::manager_cmd::{ManagerCmd, ManagerCmdErrorCode};
use crate::version::release_version;

pub const SOCKET_PATH: &str = "/run/obscura.sock";

/// Serves the same protocol as [`SOCKET_PATH`], but is connectable by everyone. The service only grants access if the peer uid is a current member of the service's group in the user database, so membership changes take effect without re-login.
pub const LIVE_GROUPS_SOCKET_PATH: &str = "/run/obscura-live-groups.sock";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinuxIpcHeader {
    pub version: String,
    #[serde(default)]
    pub denied: bool,
}

const MAX_HEADER_LEN: u32 = 4096;

impl LinuxIpcHeader {
    pub async fn write<W: AsyncWriteExt + Unpin>(&self, writer: &mut W) -> std::io::Result<()> {
        let json = serde_json::to_vec(self)?;
        let len: u32 = json.len().try_into().map_err(std::io::Error::other)?;
        let mut message = Vec::with_capacity(4 + json.len());
        message.extend_from_slice(&len.to_be_bytes());
        message.extend_from_slice(&json);
        writer.write_all(&message).await
    }

    async fn read<R: AsyncReadExt + Unpin>(reader: &mut R) -> Result<Self, ()> {
        let mut len = [0u8; 4];
        reader.read_exact(&mut len).await.map_err(|error| {
            tracing::error!(message_id = "pY7wKn2V", ?error, "failed to read IPC header length");
        })?;
        let len = u32::from_be_bytes(len);
        if len > MAX_HEADER_LEN {
            tracing::error!(message_id = "eJ5wQc8N", len, "IPC header length exceeds limit");
            return Err(());
        }
        let mut json = vec![0; u32_into_usize(len)];
        reader.read_exact(&mut json).await.map_err(|error| {
            tracing::error!(message_id = "aY3fVm7T", ?error, "failed to read IPC header");
        })?;
        serde_json::from_slice(&json).map_err(|error| {
            tracing::error!(message_id = "rD6kXb2S", ?error, "failed to parse IPC header");
        })
    }
}

#[derive(Debug)]
pub enum LinuxIpcError {
    InsufficientPermissions,
    NoListener,
    VersionMismatch { service_version: String, app_version: String },
    Other,
}

pub async fn run_command<O: DeserializeOwned>(cmd: ManagerCmd) -> Result<Result<O, ManagerCmdErrorCode>, LinuxIpcError> {
    let (mut stream, header) = match connect_and_read_header(SOCKET_PATH).await {
        Err(LinuxIpcError::InsufficientPermissions) => {
            tracing::info!(
                message_id = "wY3sKf8D",
                "insufficient permissions to use main socket, retrying live-groups socket"
            );
            connect_and_read_header(LIVE_GROUPS_SOCKET_PATH).await.map_err(|error| {
                tracing::warn!(
                    message_id = "kQ7vXn2G",
                    ?error,
                    "live-groups socket fallback failed, reporting original permission error"
                );
                LinuxIpcError::InsufficientPermissions
            })
        }
        result => result,
    }?;
    if header.denied {
        tracing::warn!(message_id = "gM4tVb7R", "service ipc header indicates denial");
        return Err(LinuxIpcError::InsufficientPermissions);
    }
    if header.version != release_version() {
        tracing::error!(
            message_id = "sQ8bHn4Z",
            service_version = header.version,
            client_version = release_version(),
            "IPC header version does not match this binary"
        );
        return Err(LinuxIpcError::VersionMismatch { service_version: header.version, app_version: release_version().to_owned() });
    }

    let json_cmd = serde_json::to_vec(&cmd).map_err(|error| {
        tracing::error!(message_id = "AdBGoG5S", ?error, "failed to serialize command");
        LinuxIpcError::Other
    })?;
    let len: u32 = json_cmd.len().try_into().map_err(|_| {
        tracing::error!(message_id = "Vq8mXpL2", "command too large to send");
        LinuxIpcError::Other
    })?;
    stream.write_all(&len.to_be_bytes()).await.map_err(|error| {
        tracing::error!(message_id = "GYCVPD3t", ?error, "failed to write length of json command");
        LinuxIpcError::Other
    })?;
    stream.write_all(json_cmd.as_slice()).await.map_err(|error| {
        tracing::error!(message_id = "FGduR73M", ?error, "failed to send json command");
        LinuxIpcError::Other
    })?;

    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.map_err(|error| {
        tracing::error!(message_id = "pdkSRS95", ?error, "failed to receive json command response");
        LinuxIpcError::Other
    })?;
    stream.shutdown().await.map_err(|error| {
        tracing::error!(message_id = "SqVcXJe4", ?error, "failed to close write end of socket stream");
        LinuxIpcError::Other
    })?;

    serde_json::from_slice(&response).map_err(|error| {
        tracing::error!(
            message_id = "2TVuEG5e",
            ?error,
            response = &*String::from_utf8_lossy(&response),
            "failed to parse json command response"
        );
        LinuxIpcError::Other
    })
}

async fn connect_and_read_header(socket_path: &str) -> Result<(UnixStream, LinuxIpcHeader), LinuxIpcError> {
    let mut stream = UnixStream::connect(socket_path).await.map_err(|error| {
        tracing::warn!(message_id = "RJEP2IV5", ?error, socket_path, "failed to connect to socket");
        match error.kind() {
            ErrorKind::NotFound => LinuxIpcError::NoListener,
            ErrorKind::PermissionDenied => LinuxIpcError::InsufficientPermissions,
            ErrorKind::ConnectionRefused => LinuxIpcError::NoListener,
            _ => LinuxIpcError::Other,
        }
    })?;

    let header = LinuxIpcHeader::read(&mut stream).await.map_err(|()| LinuxIpcError::Other)?;
    Ok((stream, header))
}

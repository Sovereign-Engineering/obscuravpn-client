use crate::client::client_error::ClientError;
use crate::service::os::linux::ipc::SOCKET_PATH;
use obscuravpn_client::manager_cmd::{ManagerCmd, ManagerCmdErrorCode};
use serde::de::DeserializeOwned;
use std::io::ErrorKind;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

pub async fn run_command<O: DeserializeOwned>(cmd: ManagerCmd) -> Result<Result<O, ManagerCmdErrorCode>, ClientError> {
    let mut stream = UnixStream::connect(SOCKET_PATH).await.map_err(|error| {
        tracing::error!(message_id = "RJEP2IV5", ?error, "failed to connect to socket: {}", error);
        match error.kind() {
            ErrorKind::NotFound => ClientError::NoService,
            ErrorKind::PermissionDenied => ClientError::InsufficientPermissions,
            ErrorKind::ConnectionRefused => ClientError::NoService,
            _ => anyhow::Error::new(error).context("failed to connect to socket").into(),
        }
    })?;

    let json_cmd = serde_json::to_vec(&cmd).map_err(|error| {
        tracing::error!(message_id = "AdBGoG5S", ?error, "failed to serialize command: {error}");
        ManagerCmdErrorCode::Other
    })?;
    let len: u32 = json_cmd.len().try_into().map_err(|_| {
        tracing::error!(message_id = "Vq8mXpL2", "command too large to send");
        ManagerCmdErrorCode::Other
    })?;
    stream.write_all(&len.to_be_bytes()).await.map_err(|error| {
        tracing::error!(message_id = "GYCVPD3t", ?error, "failed to write length of json command: {error}");
        ManagerCmdErrorCode::Other
    })?;
    stream.write_all(json_cmd.as_slice()).await.map_err(|error| {
        tracing::error!(message_id = "FGduR73M", ?error, "failed to send json command: {error}");
        ManagerCmdErrorCode::Other
    })?;
    let mut response = Vec::new();
    stream.read_to_end(&mut response).await.map_err(|error| {
        tracing::error!(message_id = "pdkSRS95", ?error, "failed to receive json command response: {error}");
        ManagerCmdErrorCode::Other
    })?;
    stream.shutdown().await.map_err(|error| {
        tracing::error!(message_id = "SqVcXJe4", ?error, "failed to close write end of socket stream: {error}");
        ManagerCmdErrorCode::Other
    })?;

    Ok(serde_json::from_slice(&response).map_err(|error| {
        tracing::error!(
            message_id = "2TVuEG5e",
            ?error,
            response = String::from_utf8_lossy(&response).as_ref(),
            "failed to parse json command response: {error}"
        );
        ManagerCmdErrorCode::Other
    })?)
}

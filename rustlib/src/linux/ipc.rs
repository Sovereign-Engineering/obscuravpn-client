use nix::unistd::{Gid, Group, User, getuid};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::env::args;
use std::io::ErrorKind;
use std::iter::once;
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

use super::argv0;
use crate::int_helper::u32_into_usize;
use crate::manager_cmd::{ManagerCmd, ManagerCmdErrorCode};
use crate::version::release_version;

pub const SOCKET_PATH: &str = "/run/obscura.sock";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinuxIpcHeader {
    pub version: String,
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
    let mut stream = UnixStream::connect(SOCKET_PATH).await.map_err(|error| {
        tracing::warn!(message_id = "RJEP2IV5", ?error, "failed to connect to socket");
        match error.kind() {
            ErrorKind::NotFound => LinuxIpcError::NoListener,
            ErrorKind::PermissionDenied => LinuxIpcError::InsufficientPermissions,
            ErrorKind::ConnectionRefused => LinuxIpcError::NoListener,
            _ => LinuxIpcError::Other,
        }
    })?;

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

    let header = LinuxIpcHeader::read(&mut stream).await.map_err(|()| LinuxIpcError::Other)?;
    if header.version != release_version() {
        tracing::error!(
            message_id = "sQ8bHn4Z",
            service_version = header.version,
            client_version = release_version(),
            "IPC header version does not match this binary"
        );
        return Err(LinuxIpcError::VersionMismatch { service_version: header.version, app_version: release_version().to_owned() });
    }

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

// Tests if IPC fails due to insufficient permissions and if this can be resolved by refreshing a group membership. If that's the case, the process is replaced by a new one, which first updates the group memberships and then reruns the current command.
pub async fn try_group_refresh_fix() {
    match run_command::<()>(ManagerCmd::Ping {}).await {
        Err(LinuxIpcError::InsufficientPermissions) => tracing::debug!(
            message_id = "t4O1pv8K",
            "insufficient permissions for IPC commands, check if IPC works in a new shell"
        ),
        Ok(Ok(())) => {
            tracing::debug!(message_id = "ZA5DS6pc", "IPC test succeeded, group refresh not necessary");
            return;
        }
        Ok(Err(error)) => {
            tracing::debug!(message_id = "hV1KZ8pw", ?error, "IPC ping returned error, not attempting group refresh");
            return;
        }
        Err(error) => {
            tracing::debug!(
                message_id = "EP7be96J",
                ?error,
                "IPC test failed, but not due to insufficient permissions, not attempting group refresh"
            );
            return;
        }
    }

    tokio::task::spawn_blocking(|| {
        let user = match User::from_uid(getuid()) {
            Ok(Some(user)) => user,
            Err(error) => {
                tracing::error!(message_id = "YBoOFOh1", ?error, "failed to resolve uid to user");
                return;
            }
            Ok(None) => {
                tracing::error!(message_id = "ccq4YLw9", "current user does not exist");
                return;
            }
        };

        let group = match std::fs::metadata(SOCKET_PATH) {
            Ok(meta) => match Group::from_gid(Gid::from_raw(meta.gid())) {
                Ok(Some(group)) => group,
                Err(error) => {
                    tracing::error!(message_id = "bm2pO7u5", ?error, "failed to resolve socket gid to group");
                    return;
                }
                Ok(None) => {
                    tracing::error!(message_id = "UyCY58ay", "socket group does not exist");
                    return;
                }
            },
            Err(error) => {
                tracing::error!(message_id = "iZaf0n3l", ?error, "failed to look up socket metadata");
                return;
            }
        };

        // sg may ask for a password interactively if the user is not a member of the group, so we check manually
        if group.mem.iter().all(|membership| *membership != user.name) {
            tracing::error!(message_id = "7PswELBV", "user is not a member of {:?}", group.name);
            return;
        }

        let Some(invocation_path) = argv0() else {
            tracing::error!(message_id = "NR6Vra8m", "cannot restart process without argv[0]");
            return;
        };
        let Some(invocation_path) = invocation_path.to_str() else {
            tracing::error!(message_id = "xhz9ATa6", "argv[0] is not valid UTF8");
            return;
        };

        // adding this sentinel flag to all invocations make sure this logic never triggers recursively
        const NO_PERMISSION_FIX_ARG: &str = "--no-group-refresh";

        let Ok(mut command) = build_sg_exec_cmd(&group.name, invocation_path, [NO_PERMISSION_FIX_ARG, "ipc-test"]).inspect_err(|error| {
            tracing::error!(
                message_id = "TSjQoNIW",
                ?error,
                "failed to quote ipc test command for execution in new shell"
            )
        }) else {
            return;
        };
        match command.status() {
            Ok(exit_status) => {
                if exit_status.success() {
                    tracing::debug!(message_id = "RYFtF944", "IPC succeeded in new shell");
                } else {
                    tracing::debug!(message_id = "GnTSEqyU", "IPC failed in new shell");
                    return;
                }
            }
            Err(error) => {
                tracing::error!(message_id = "hdrXBHqC", ?error, "failed to run ipc test in new shell");
                return;
            }
        }

        tracing::info!(message_id = "6I3WIrPh", "group refresh required, restarting process in a new shell");
        let current_args: Vec<String> = args().skip(1).collect();
        let new_args_iter = once(NO_PERMISSION_FIX_ARG).chain(current_args.iter().map(String::as_str));
        let Ok(mut command) = build_sg_exec_cmd(&group.name, invocation_path, new_args_iter).inspect_err(|error| {
            tracing::error!(
                message_id = "DD0zPnz8",
                ?error,
                "failed to quote current command for execution in new shell"
            )
        }) else {
            return;
        };
        let error = command.exec();
        tracing::error!(
            message_id = "u8h0TXml",
            ?error,
            "failed to replace current process with same command in new shell"
        );
    })
    .await
    .unwrap()
}

// sg takes the command as a single argument. To make sure the command survives the subsequent splitting unharmed, this function ensures the command and its arguments are correctly quoted and escaped.
fn build_sg_exec_cmd<'a>(
    group_name: &str,
    program: &'a str,
    args: impl IntoIterator<Item = &'a str>,
) -> Result<std::process::Command, shlex::QuoteError> {
    let exec_cmd = once("exec").chain(once(program)).chain(args);
    let sg_command_arg = shlex::try_join(exec_cmd)?;
    let mut cmd = std::process::Command::new("sg");
    cmd.arg(group_name);
    cmd.arg("-c");
    cmd.arg(sg_command_arg);
    Ok(cmd)
}

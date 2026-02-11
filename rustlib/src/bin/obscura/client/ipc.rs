use crate::ClientIpcTestArgs;
use crate::client::client_error::ClientError;
use crate::service::os::linux::ipc::SOCKET_PATH;
use anyhow::anyhow;
use nix::unistd::Gid;
use obscuravpn_client::manager_cmd::{ManagerCmd, ManagerCmdErrorCode};
use serde::de::DeserializeOwned;
use std::io::ErrorKind;
use std::iter::once;
use std::os::unix::fs::MetadataExt;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

pub async fn run_command<O: DeserializeOwned>(cmd: ManagerCmd) -> Result<Result<O, ManagerCmdErrorCode>, ClientError> {
    let mut stream = UnixStream::connect(SOCKET_PATH).await.map_err(|error| {
        tracing::warn!(message_id = "RJEP2IV5", ?error, "failed to connect to socket: {}", error);
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
            response = &*String::from_utf8_lossy(&response),
            "failed to parse json command response: {error}"
        );
        ManagerCmdErrorCode::Other
    })?)
}

pub async fn ipc_test(_: ClientIpcTestArgs) -> Result<(), ClientError> {
    if let Err(error) = run_command::<()>(ManagerCmd::Ping {}).await? {
        tracing::error!(message_id = "my5QZfPB", ?error, "IPC ping returned error");
        return Err(anyhow!("IPC ping returned error").into());
    }
    Ok(())
}

// Tests if IPC fails due to insufficient permissions and if this can be resolved by refreshing a group membership. If that's the case, the process is replaced by a new one, which first updates the group memberships and then reruns the current command.
pub async fn try_group_refresh_fix() {
    match ipc_test(ClientIpcTestArgs {}).await {
        Err(ClientError::InsufficientPermissions) => tracing::debug!(
            message_id = "t4O1pv8K",
            "insufficient permissions for IPC commands, check if IPC works in a new shell"
        ),
        Ok(_) => {
            tracing::debug!(message_id = "ZA5DS6pc", "IPC test succeeded, group refresh not necessary");
            return;
        }
        Err(err) => {
            tracing::debug!(
                message_id = "EP7be96J",
                ?err,
                "IPC test failed, but not due to insufficient permissions, not attempting group refresh: {err}"
            );
            return;
        }
    }

    tokio::task::spawn_blocking(|| {
        use nix::unistd::{Group, User, getuid};
        use std::env::{args, current_exe};
        use std::os::unix::process::CommandExt;

        let user = match User::from_uid(getuid()) {
            Ok(Some(user)) => user,
            Err(error) => {
                tracing::error!(message_id = "YBoOFOh1", ?error, "failed to resolve uid to user: {error}");
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
                    tracing::error!(message_id = "bm2pO7u5", ?error, "failed to resolve socket gid to group: {error}");
                    return;
                }
                Ok(None) => {
                    tracing::error!(message_id = "UyCY58ay", "socket group does not exist");
                    return;
                }
            },
            Err(error) => {
                tracing::error!(message_id = "iZaf0n3l", ?error, "failed to look up socket metadata: {error}");
                return;
            }
        };

        // sg may ask for a password interactively if the user is not a member of the group, so we check manually
        if group.mem.iter().all(|membership| *membership != user.name) {
            tracing::error!(message_id = "7PswELBV", "user is not a member of {:?}", group.name);
            return;
        }

        let Ok(current_exe) = current_exe()
            .inspect_err(|error| tracing::error!(message_id = "NR6Vra8m", ?error, "failed to identify current executable path: {error}"))
        else {
            return;
        };
        let Some(current_exe) = current_exe.to_str() else {
            tracing::error!(message_id = "xhz9ATa6", "current executable path is not valid UTF8");
            return;
        };

        // adding this sentinel flag to all invocations make sure this logic never triggers recursively
        const NO_PERMISSION_FIX_ARG: &str = "--no-group-refresh";

        let Ok(mut command) = build_sg_exec_cmd(&group.name, current_exe, [NO_PERMISSION_FIX_ARG, "ipc-test"]).inspect_err(|error| {
            tracing::error!(
                message_id = "TSjQoNIW",
                ?error,
                "failed to quote ipc test command for execution in new shell: {error}"
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
                tracing::error!(message_id = "hdrXBHqC", ?error, "failed to run ipc test in new shell: {error}");
            }
        }

        tracing::info!(message_id = "6I3WIrPh", "group refresh required, restarting process in a new shell");
        let current_args: Vec<String> = args().skip(1).collect();
        let new_args_iter = once(NO_PERMISSION_FIX_ARG).chain(current_args.iter().map(String::as_str));
        let Ok(mut command) = build_sg_exec_cmd(&group.name, current_exe, new_args_iter).inspect_err(|error| {
            tracing::error!(
                message_id = "TSjQoNIW",
                ?error,
                "failed to quote current command for execution in new shell: {error}"
            )
        }) else {
            return;
        };
        let error = command.exec();
        tracing::error!(
            message_id = "u8h0TXml",
            ?error,
            "failed to replace current process with same command in new shell: {error}"
        );
    })
    .await
    .unwrap()
}

// sg takes the command as a single argument. To make sure the command survives the subsequent splitting unharmed, this function ensures the command and its arguments are correctly quoted and escaped.
fn build_sg_exec_cmd<'a>(
    group_name: &str,
    exe: &'a str,
    args: impl IntoIterator<Item = &'a str>,
) -> Result<std::process::Command, shlex::QuoteError> {
    let exec_cmd = once("exec").chain(once(exe)).chain(args);
    let sg_command_arg = shlex::try_join(exec_cmd)?;
    let mut cmd = std::process::Command::new("sg");
    cmd.arg(group_name);
    cmd.arg("-c");
    cmd.arg(sg_command_arg);
    Ok(cmd)
}

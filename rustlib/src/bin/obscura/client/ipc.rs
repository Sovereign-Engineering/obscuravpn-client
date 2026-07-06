use crate::ClientIpcTestArgs;
use anyhow::anyhow;
use obscuravpn_client::linux::{ClientError, run_command};
use obscuravpn_client::manager_cmd::ManagerCmd;

pub async fn ipc_test(_: ClientIpcTestArgs) -> Result<(), ClientError> {
    if let Err(error) = run_command::<()>(ManagerCmd::Ping {}).await? {
        tracing::error!(message_id = "my5QZfPB", ?error, "IPC ping returned error");
        return Err(anyhow!("IPC ping returned error").into());
    }
    Ok(())
}

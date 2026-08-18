use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

const OUTPUT_LIMIT: u64 = 64 * 1024;

#[derive(Debug, Serialize, Deserialize)]
pub struct DebugTaskCommand {
    pub program: String,
    pub args: Vec<String>,
    pub exit_status: DebugTaskCommandExitStatus,
    pub stdout: String,
    pub stdout_truncated_bytes: Option<u64>,
    pub stderr: String,
    pub stderr_truncated_bytes: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DebugTaskCommandExitStatus {
    Code(i32),
    Signal(i32),
}

impl DebugTaskCommandExitStatus {
    fn from_status(status: std::process::ExitStatus) -> Result<Self, std::io::Error> {
        if let Some(code) = status.code() {
            return Ok(Self::Code(code));
        }
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt as _;
            if let Some(signal) = status.signal() {
                return Ok(Self::Signal(signal));
            }
        }
        Err(std::io::Error::other("no exit code or signal"))
    }
}

impl DebugTaskCommand {
    pub async fn run(program: String, args: Vec<String>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut child = Command::new(&program)
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let stdout_pipe = child.stdout.take().ok_or("missing stdout pipe")?;
        let stderr_pipe = child.stderr.take().ok_or("missing stderr pipe")?;
        let wait = async { DebugTaskCommandExitStatus::from_status(child.wait().await?) };
        let (exit_status, stdout, stderr) = tokio::join!(wait, read_capped(stdout_pipe), read_capped(stderr_pipe));
        let (stdout, stdout_truncated_bytes) = stdout?;
        let (stderr, stderr_truncated_bytes) = stderr?;
        Ok(Self {
            program,
            args,
            exit_status: exit_status?,
            stdout,
            stdout_truncated_bytes,
            stderr,
            stderr_truncated_bytes,
        })
    }
}

async fn read_capped(mut pipe: impl AsyncRead + Unpin) -> Result<(String, Option<u64>), std::io::Error> {
    let mut kept = Vec::new();
    (&mut pipe).take(OUTPUT_LIMIT).read_to_end(&mut kept).await?;
    let dropped = tokio::io::copy(&mut pipe, &mut tokio::io::sink()).await?;
    let truncated = (dropped > 0).then_some(dropped);
    Ok((String::from_utf8_lossy(&kept).into_owned(), truncated))
}

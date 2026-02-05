use super::{RUNTIME, get_manager};
use crate::{quicwg::TUNNEL_MTU, tokio::AbortOnDrop};
use anyhow::Context as _;
use nix::{errno::Errno, unistd};
use std::os::fd::{AsRawFd as _, OwnedFd};
use tokio::io::unix::AsyncFd;

pub struct Tun {
    fd: OwnedFd,
    _read_loop_task: AbortOnDrop,
}

impl Tun {
    pub fn spawn(fd: OwnedFd) -> anyhow::Result<Self> {
        let fd_watcher = AsyncFd::new(fd.as_raw_fd()).context("failed to watch tun")?;
        let manager = get_manager()?.clone();
        let read_loop_task = RUNTIME.spawn(async move {
            let mut buf = Box::new([0; TUNNEL_MTU as _]);
            loop {
                match fd_watcher.readable().await {
                    Ok(mut guard) => match unistd::read(&fd_watcher, &mut buf[..]) {
                        Ok(n) => {
                            if n > 0 {
                                manager.send_packet(&mut buf[..n]);
                            }
                        }
                        Err(Errno::EAGAIN) => {
                            guard.clear_ready();
                        }
                        Err(error) => {
                            tracing::error!(message_id = "eagh6Noh", ?error, "failed to read from tun");
                            break;
                        }
                    },
                    Err(error) => {
                        tracing::error!(message_id = "r5N6izcO", ?error, "failed to wait for tun to become readable");
                        break;
                    }
                }
            }
        });
        Ok(Self { fd, _read_loop_task: read_loop_task.into() })
    }

    pub fn write(&self, packet: &[u8]) {
        if packet.len() > TUNNEL_MTU as usize {
            tracing::warn!(message_id = "Yc1WxQBY", packet_len = packet.len(), "packet larger than MTU",);
        }
        if let Err(error) = unistd::write(&self.fd, packet) {
            tracing::error!(message_id = "W0sOhigq", ?error, "writing packet to tun failed");
        }
    }
}

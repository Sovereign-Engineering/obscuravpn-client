use anyhow::Context;
use async_trait::async_trait;
use std::net::IpAddr;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::mpsc::{Receiver, Sender, channel, error::TrySendError},
};
#[async_trait]
pub trait Virtual {
    async fn get_packet(&mut self) -> anyhow::Result<Vec<u8>>;
    async fn put_packet(&mut self, packet: &[u8]) -> anyhow::Result<()>;
}

pub struct TunDevice {
    tun: tun::AsyncDevice,
    buf: Vec<u8>,
}

impl TunDevice {
    pub fn create(addr: IpAddr) -> anyhow::Result<Self> {
        let mut config = tun::Configuration::default();
        config.name("obscura").address(addr).mtu(1280).up();
        let tun = tun::create_as_async(&config)?;
        let buf = vec![0u8; u16::MAX as usize];
        Ok(Self { tun, buf })
    }
    pub fn from_raw_fd(fd: std::os::fd::RawFd) -> anyhow::Result<Self> {
        let mut config = tun::Configuration::default();
        config.raw_fd(fd);
        let tun = tun::create_as_async(&config)?;
        let buf = vec![0u8; u16::MAX as usize];
        Ok(Self { tun, buf })
    }
}

#[async_trait]
impl Virtual for TunDevice {
    async fn get_packet(&mut self) -> anyhow::Result<Vec<u8>> {
        let n = self.tun.read(&mut self.buf).await?;
        Ok(self.buf[0..n].to_vec())
    }

    async fn put_packet(&mut self, packet: &[u8]) -> anyhow::Result<()> {
        self.tun.write_all(packet).await?;
        Ok(())
    }
}

pub struct ChannelDevice {
    get: Receiver<Vec<u8>>,
    put: Sender<Vec<u8>>,
}

impl ChannelDevice {
    pub fn create() -> (Self, Sender<Vec<u8>>, Receiver<Vec<u8>>) {
        let (get_send, get_recv) = channel(1024);
        let (put_send, put_recv) = channel(1024);
        (Self { get: get_recv, put: put_send }, get_send, put_recv)
    }
}

#[async_trait]
impl Virtual for ChannelDevice {
    async fn get_packet(&mut self) -> anyhow::Result<Vec<u8>> {
        self.get.recv().await.context("sender closed")
    }

    async fn put_packet(&mut self, packet: &[u8]) -> anyhow::Result<()> {
        if let Err(err) = self.put.try_send(packet.into()) {
            match err {
                TrySendError::Full(_) => {}
                TrySendError::Closed(_) => anyhow::bail!("receiver closed"),
            }
        }
        Ok(())
    }
}

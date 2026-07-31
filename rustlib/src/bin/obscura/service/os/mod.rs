#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "windows")]
pub mod windows;

pub const MAX_IPC_MESSAGE_LEN: u32 = 1_000_000;

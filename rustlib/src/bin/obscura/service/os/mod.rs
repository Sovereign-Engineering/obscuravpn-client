#[cfg(target_os = "linux")]
pub mod linux;
#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub type OsTunWriterImpl = windows::tun::TunWriter;

pub mod client_state;
pub mod config;
pub mod errors;
pub mod net;
pub mod network_config;
pub mod quicwg;
mod serde_safe;
pub mod virt;

#[cfg(target_os = "macos")]
pub mod apple;

pub const DEFAULT_API_URL: &str = "https://v1.api.prod.obscura.net/api";

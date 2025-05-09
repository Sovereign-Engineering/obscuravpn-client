#![allow(clippy::large_enum_variant, clippy::too_many_arguments)]

pub mod client_state;
pub mod config;
pub mod errors;
pub mod exit_selection;
mod ffi_helpers;
mod manager;
mod manager_cmd;
pub mod net;
pub mod network_config;
pub mod quicwg;
pub mod relay_selection;
mod serde_safe;
pub mod tunnel_state;
pub mod virt;

#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod apple;
mod cached_value;

pub const DEFAULT_API_URL: &str = "https://v1.api.prod.obscura.net/api";

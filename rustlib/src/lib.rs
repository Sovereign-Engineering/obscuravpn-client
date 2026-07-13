#[macro_use]
pub mod rate_limited_log;

pub mod backoff;
pub mod client_state;
pub mod config;
pub mod errors;
pub mod exit_selection;
pub mod ffi_helpers;
pub mod manager;
pub mod manager_cmd;
pub mod net;
pub mod network_config;
pub mod quicwg;
pub mod relay_selection;
mod serde_safe;
pub mod tokio;
pub mod tunnel_state;

#[cfg(test)]
mod backoff_test;

#[cfg(target_os = "android")]
pub mod android;
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub mod apple;
mod cached_value;
mod constants;
pub mod debug_bundle;
mod dns;
#[cfg(target_os = "linux")]
pub mod linux;
mod liveness;
pub mod local_network;
pub mod logging;
pub mod os;
pub mod positive_u31;
mod wake_instant;

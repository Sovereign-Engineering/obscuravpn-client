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
mod liveness;
mod logging;
pub mod positive_u31;

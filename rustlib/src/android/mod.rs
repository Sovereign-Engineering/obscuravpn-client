mod class_cache;
mod ffi;
mod future;
mod tunnel;
mod util;

use crate::manager::Manager;
use anyhow::Context as _;
use std::sync::{Arc, LazyLock, OnceLock};
use tokio::runtime::Runtime;

static RUNTIME: LazyLock<Runtime> = LazyLock::new(|| Runtime::new().unwrap());
static MANAGER: OnceLock<Arc<Manager>> = OnceLock::new();

fn get_manager() -> anyhow::Result<&'static Arc<Manager>> {
    MANAGER.get().context("global FFI manager not initialized")
}

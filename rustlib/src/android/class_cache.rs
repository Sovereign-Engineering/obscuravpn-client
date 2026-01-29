use super::util::throw_runtime_exception;
use anyhow::Context as _;
use jni::{
    JNIEnv,
    objects::{GlobalRef, JClass},
};
use std::sync::OnceLock;

static CLASS_CACHE: OnceLock<ClassCache> = OnceLock::new();

#[derive(Debug)]
pub struct ClassCache {
    json_ffi_exception: GlobalRef,
}

impl ClassCache {
    fn new(env: &mut JNIEnv) -> anyhow::Result<Self> {
        let json_ffi_exception = env.find_class("net/obscura/vpnclientapp/client/JsonFfiException")?;
        let json_ffi_exception = env.new_global_ref(json_ffi_exception)?;
        Ok(Self { json_ffi_exception })
    }

    pub fn json_ffi_exception(&self) -> &JClass<'static> {
        self.json_ffi_exception.as_obj().into()
    }
}

pub fn init(env: &mut JNIEnv) {
    // `JNI_OnLoad` could be called multiple times, so unlike with manager init,
    // we won't throw an exception if this is called multiple times.
    // https://issuetracker.google.com/issues/220523932
    CLASS_CACHE.get_or_init(|| {
        // We can remove this panic once `get_or_try_init` is stable:
        // https://github.com/rust-lang/rust/issues/109737
        ClassCache::new(env).expect("failed to create class cache")
    });
}

pub fn get() -> anyhow::Result<&'static ClassCache> {
    CLASS_CACHE.get().context("global class cache not initialized")
}

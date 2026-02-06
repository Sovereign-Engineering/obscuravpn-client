use anyhow::Context as _;
use jni::{
    JNIEnv,
    objects::{GlobalRef, JClass},
};
use once_cell::sync::OnceCell;

static CLASS_CACHE: OnceCell<ClassCache> = OnceCell::new();

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

pub fn init(env: &mut JNIEnv) -> anyhow::Result<()> {
    // `JNI_OnLoad` could be called multiple times, so unlike with manager init,
    // we won't throw an exception if this is called multiple times.
    // https://issuetracker.google.com/issues/220523932
    CLASS_CACHE
        .get_or_try_init(|| ClassCache::new(env).context("failed to create class cache"))
        .map(drop)
}

pub fn get() -> anyhow::Result<&'static ClassCache> {
    CLASS_CACHE.get().context("global class cache not initialized")
}

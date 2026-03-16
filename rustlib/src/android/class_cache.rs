use jni::{
    JNIEnv,
    objects::{GlobalRef, JClass},
};

#[derive(Debug)]
pub struct ClassCache {
    json_ffi_exception: GlobalRef,
    vpn_service: GlobalRef,
}

impl ClassCache {
    /// Looking up app-specific Java classes from native threads isn't possible, so we cache all the app-specific classes we need.
    /// Must be called on a Java thread.
    /// https://developer.android.com/ndk/guides/jni-tips#faq:-why-didnt-findclass-find-my-class
    pub fn new(env: &mut JNIEnv) -> anyhow::Result<Self> {
        let json_ffi_exception = env.find_class("net/obscura/vpnclientapp/client/JsonFfiException")?;
        let json_ffi_exception = env.new_global_ref(json_ffi_exception)?;
        let vpn_service = env.find_class("net/obscura/vpnclientapp/services/ObscuraVpnService")?;
        let vpn_service = env.new_global_ref(vpn_service)?;
        Ok(Self { json_ffi_exception, vpn_service })
    }

    pub fn json_ffi_exception(&self) -> &JClass<'static> {
        self.json_ffi_exception.as_obj().into()
    }

    pub fn vpn_service(&self) -> &JClass<'static> {
        self.vpn_service.as_obj().into()
    }
}

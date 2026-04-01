use jni::{
    JNIEnv,
    objects::{GlobalRef, JClass},
};
use std::sync::Arc;

#[derive(Debug)]
pub struct ClassCache {
    ffi_handle: GlobalRef,
    error_code_exception: GlobalRef,
    vpn_service: GlobalRef,
}

impl ClassCache {
    /// Looking up app-specific Java classes from native threads isn't possible, so we cache all the app-specific classes we need.
    /// Must be called on a Java thread.
    /// https://developer.android.com/ndk/guides/jni-tips#faq:-why-didnt-findclass-find-my-class
    pub fn new(env: &mut JNIEnv) -> anyhow::Result<Arc<Self>> {
        let ffi_handle = env.find_class("net/obscura/vpnclientapp/client/ObscuraLibrary$FfiHandle")?;
        let ffi_handle = env.new_global_ref(ffi_handle)?;
        let error_code_exception = env.find_class("net/obscura/vpnclientapp/client/ErrorCodeException")?;
        let error_code_exception = env.new_global_ref(error_code_exception)?;
        let vpn_service = env.find_class("net/obscura/vpnclientapp/services/ObscuraVpnService")?;
        let vpn_service = env.new_global_ref(vpn_service)?;
        Ok(Arc::new(Self { ffi_handle, error_code_exception, vpn_service }))
    }

    pub fn ffi_handle(&self) -> &JClass<'static> {
        self.ffi_handle.as_obj().into()
    }

    pub fn error_code_exception(&self) -> &JClass<'static> {
        self.error_code_exception.as_obj().into()
    }

    pub fn vpn_service(&self) -> &JClass<'static> {
        self.vpn_service.as_obj().into()
    }
}

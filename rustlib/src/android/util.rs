use anyhow::Context as _;
use camino::Utf8Path;
use jni::{JNIEnv, objects::JString, strings::JavaStr};
use std::{borrow::Cow, ffi::CStr, fmt::Display};

pub fn throw_runtime_exception(env: &mut JNIEnv, msg: impl Display) {
    let msg = msg.to_string();
    if let Err(error) = env.throw_new("java/lang/RuntimeException", &msg) {
        tracing::error!(message_id = "bxCfsHAC", ?error, msg, "failed to throw `RuntimeException`");
    }
}

/// RAII handle that provides a UTF-8 view into a `java.lang.String`.
pub struct Utf8JavaStr<'a, 'b> {
    s: Cow<'a, str>,
    obj: &'a JString<'a>,
    env: JNIEnv<'b>,
}

impl<'a, 'b> Utf8JavaStr<'a, 'b> {
    /// `name` is only used for error messages.
    pub fn new(env: &mut JNIEnv<'b>, obj: &'a JString<'a>, name: &str) -> anyhow::Result<Self> {
        // We unfortunately can't safely use `get_string_unchecked`, since the
        // Java/Kotlin build will still succeed even if we're passed an argument
        // of the wrong type for our function signatures.
        //
        // Either method is really just `GetStringUTFChars`, which converts from
        // UTF-16 to Modified UTF-8 (this is the only unavoidable alloc):
        // https://developer.android.com/ndk/guides/jni-tips#utf-8-and-utf-16-strings
        let java_str = env.get_string(obj).with_context(|| format!("{name:?} wasn't a `java.lang.String`"))?;
        // Leak the result of `GetStringUTFChars`
        let ptr = java_str.into_raw();
        // SAFETY: We've taken ownership of the result of `GetStringUTFChars`,
        // and it's null-terminated
        // (This uses the same lifetime as obj, since the obj is needed to
        // release the underlying memory later)
        let c_str = unsafe { CStr::from_ptr(ptr) };
        // The Modified UTF-8 returned by `GetStringUTFChars` will be valid
        // UTF-8 for anything in the Basic Multilingual Plane, so this will
        // almost never need to allocate:
        // https://en.wikipedia.org/wiki/CESU-8
        let s = cesu8::from_java_cesu8(c_str.to_bytes()).with_context(|| format!("{name:?} couldn't be converted to UTF-8"))?;
        // SAFETY: Only used to release refs
        let env = unsafe { env.unsafe_clone() };
        Ok(Self { s, obj, env })
    }

    pub fn as_str(&self) -> &str {
        self.s.as_ref()
    }

    pub fn as_path(&self) -> &Utf8Path {
        Utf8Path::new(self.as_str())
    }
}

impl<'a, 'b> Drop for Utf8JavaStr<'a, 'b> {
    fn drop(&mut self) {
        // Release the result of `GetStringUTFChars`
        // SAFETY: ptr came from `JavaStr::into_raw` and this is the same obj
        // used to construct that `JavaStr`
        unsafe { JavaStr::from_raw(&self.env, self.obj, self.s.as_ptr() as *const _) };
    }
}

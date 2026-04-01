use super::class_cache::ClassCache;
use crate::manager_cmd::{ManagerCmdErrorCode, ManagerCmdOk};
use anyhow::Context as _;
use jni::{
    JNIEnv,
    objects::{JObject, JValue},
};

pub fn signal_json_ffi_future(
    class_cache: &ClassCache,
    env: &mut JNIEnv,
    j_future: &JObject,
    result: Result<ManagerCmdOk, ManagerCmdErrorCode>,
) -> anyhow::Result<()> {
    match result.and_then(|ok| {
        serde_json::to_string(&ok).map_err(|error| {
            tracing::error!(message_id = "hP0R8zXa", ?error, "failed to serialize successful cmd result");
            ManagerCmdErrorCode::Other
        })
    }) {
        Ok(ok) => {
            let j_ok = env.new_string(&ok).map(JObject::from).unwrap_or_else(|error| {
                tracing::error!(message_id = "eeAQzxl1", ?error, "failed to convert `ok` to `JString`");
                JObject::null()
            });
            env.call_method(j_future, "complete", "(Ljava/lang/Object;)Z", &[JValue::Object(&j_ok)])
                .context("failed to call `complete`")?;
        }
        Err(error) => {
            let j_error = env.new_string(error.as_static_str()).context("failed to convert `error` to `JString`")?;
            let j_exception = env
                .new_object(
                    class_cache.error_code_exception(),
                    "(Ljava/lang/String;)V",
                    &[JValue::Object(&j_error.into())],
                )
                .context("failed to create `ErrorCodeException`")?;
            env.call_method(
                j_future,
                "completeExceptionally",
                "(Ljava/lang/Throwable;)Z",
                &[JValue::Object(&j_exception)],
            )
            .context("failed to call `completeExceptionally`")?;
        }
    }
    Ok(())
}

use crate::manager_cmd::{ManagerCmdErrorCode, ManagerCmdOk};
use anyhow::Context as _;
use jni::{
    JNIEnv,
    objects::{JObject, JValue},
};

pub fn signal_json_ffi_future(env: &mut JNIEnv, j_future: &JObject, result: Result<ManagerCmdOk, ManagerCmdErrorCode>) -> anyhow::Result<()> {
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
            let j_error = serde_json::to_string(&error)
                .context("failed to serialize failed cmd result")
                .and_then(|error| env.new_string(&error).context("failed to convert `error` to `JString`"))
                .map(JObject::from)
                .unwrap_or_else(|error| {
                    tracing::error!(message_id = "2MpsFFGe", ?error, "failed to propagate error message");
                    JObject::null()
                });
            let j_exception = env
                .new_object(
                    "net/obscura/vpnclientapp/client/JsonFfiException",
                    "(Ljava/lang/String;)V",
                    &[JValue::Object(&j_error)],
                )
                .unwrap_or_else(|error| {
                    tracing::error!(message_id = "T3tXX3yk", ?error, "failed to create `JsonFfiException`");
                    JObject::null()
                });
            env.call_method(
                j_future,
                "completeExceptionally",
                "(Ljava/lang/Throwable;)V",
                &[JValue::Object(&j_exception)],
            )
            .context("failed to call `completeExceptionally`")?;
        }
    }
    Ok(())
}

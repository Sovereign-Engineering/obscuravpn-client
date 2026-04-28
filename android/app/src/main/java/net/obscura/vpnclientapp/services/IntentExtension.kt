package net.obscura.vpnclientapp.services

import android.content.Intent
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.client.ErrorCodeException
import net.obscura.vpnclientapp.client.errorCodeOther

private val log = Logger("IntentExtension")

private const val EXTRA_ID = "id"
private const val EXTRA_VALUE = "value"
private const val EXTRA_ERROR_CODE = "errorCode"

fun Intent.putJsonFfiExtras(id: Long, value: String?, exception: Throwable?) {
    this.putExtra(EXTRA_ID, id)
    this.putExtra(EXTRA_VALUE, value)
    this.putExtra(
        EXTRA_ERROR_CODE,
        when (exception) {
            is ErrorCodeException -> exception.errorCode
            is Throwable -> {
                log.error("job $id threw unexpected exception type: $exception", tr = exception)
                null
            }
            else -> {
                if (value == null) {
                    log.error("job $id completed with no response")
                }
                null
            }
        },
    )
}

data class JsonFfiIntentPayload(val id: Long, val result: Result<String>)

fun Intent.getJsonFfiExtras(): JsonFfiIntentPayload {
    val id = this.getLongExtra(EXTRA_ID, -1)
    val value = this.getStringExtra(EXTRA_VALUE)
    val errorCode = this.getStringExtra(EXTRA_ERROR_CODE)
    return JsonFfiIntentPayload(
        id,
        if (value != null) {
            log.trace("job $id completed with value: $value")
            Result.success(value)
        } else {
            log.trace("job $id completed with error code: $errorCode")
            Result.failure(errorCode?.let { ErrorCodeException(it) } ?: errorCodeOther())
        },
    )
}

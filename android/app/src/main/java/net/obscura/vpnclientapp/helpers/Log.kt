package net.obscura.vpnclientapp.helpers

import android.util.Log
import net.obscura.vpnclientapp.client.ObscuraLibrary

enum class LogLevel { TRACE, DEBUG, INFO, WARN, ERROR }

fun forwardLog(level: LogLevel, tag: String, message: String, messageId: String?, tr: Throwable?) {
    if (ObscuraLibrary.getIsLoaded()) {
        ObscuraLibrary.forwardLog(level.ordinal, tag, message, messageId ?: "JavaNoID", tr?.toString())
    }
}

inline fun <reified T> T.logTrace(
    message: String,
    messageId: String? = null,
    tr: Throwable? = null,
) {
    Log.v(T::class.java.simpleName, message, tr)
    forwardLog(LogLevel.TRACE, T::class.java.simpleName, message, messageId, tr)
}

inline fun <reified T> T.logDebug(
    message: String,
    messageId: String? = null,
    tr: Throwable? = null,
) {
    Log.d(T::class.java.simpleName, message, tr)
    forwardLog(LogLevel.DEBUG, T::class.java.simpleName, message, messageId, tr)
}

inline fun <reified T> T.logInfo(
    message: String,
    messageId: String? = null,
    tr: Throwable? = null,
) {
    Log.i(T::class.java.simpleName, message, tr)
    forwardLog(LogLevel.INFO, T::class.java.simpleName, message, messageId, tr)
}

inline fun <reified T> T.logWarn(
    message: String,
    messageId: String? = null,
    tr: Throwable? = null,
) {
    Log.w(T::class.java.simpleName, message, tr)
    forwardLog(LogLevel.WARN, T::class.java.simpleName, message, messageId, tr)
}

inline fun <reified T> T.logError(
    message: String,
    messageId: String? = null,
    tr: Throwable? = null,
) {
    Log.e(T::class.java.simpleName, message, tr)
    forwardLog(LogLevel.ERROR, T::class.java.simpleName, message, messageId, tr)
}

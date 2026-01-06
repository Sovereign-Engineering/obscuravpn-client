package net.obscura.vpnclientapp.helpers

import android.util.Log

inline fun <reified T> T.logDebug(
    message: String,
    tr: Throwable? = null,
) {
  Log.d(T::class.java.simpleName, message, tr)
}

inline fun <reified T> T.logInfo(
    message: String,
    tr: Throwable? = null,
) {
  Log.i(T::class.java.simpleName, message, tr)
}

inline fun <reified T> T.logWarn(
    message: String,
    tr: Throwable? = null,
) {
  Log.w(T::class.java.simpleName, message, tr)
}

inline fun <reified T> T.logError(
    message: String,
    tr: Throwable? = null,
) {
  Log.e(T::class.java.simpleName, message, tr)
}

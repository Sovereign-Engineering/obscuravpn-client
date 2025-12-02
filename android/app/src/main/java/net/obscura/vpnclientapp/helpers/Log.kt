package net.obscura.vpnclientapp.helpers

import android.util.Log

inline fun <reified T> T.debug(
    message: String,
    tr: Throwable? = null,
) {
  Log.d(T::class.java.simpleName, message, tr)
}

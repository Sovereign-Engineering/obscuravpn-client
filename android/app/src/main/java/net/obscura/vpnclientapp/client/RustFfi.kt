package net.obscura.vpnclientapp.client

import android.content.Context
import java.util.concurrent.CompletableFuture
import kotlin.reflect.KClass
import net.obscura.lib.util.Logger

class RustFfi(context: Context, userAgent: String) {
    private val handle: ObscuraLibrary.FfiHandle =
        ObscuraLibrary.load(
            context,
            userAgent,
        )

    fun logger(tag: KClass<*>): Logger {
        return Logger(tag) { params ->
            ObscuraLibrary.forwardLog(
                params.level.ordinal,
                params.tag,
                params.message,
                params.messageId ?: "JavaNoID",
                params.tr?.toString(),
            )
        }
    }

    fun jsonFfi(json: String, future: CompletableFuture<String>) {
        ObscuraLibrary.jsonFfi(handle, json, future)
    }

    fun setNetworkInterface(name: String, index: Int) {
        ObscuraLibrary.setNetworkInterface(handle, name, index)
    }

    fun unsetNetworkInterface() {
        ObscuraLibrary.unsetNetworkInterface(handle)
    }
}

package net.obscura.vpnclientapp.ui.bridge

import android.content.Context
import android.webkit.JavascriptInterface
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.serialization.Serializable
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.ErrorCodeException
import net.obscura.vpnclientapp.client.errorCodeOther
import net.obscura.vpnclientapp.client.jsonConfig
import net.obscura.vpnclientapp.services.IObscuraVpnService
import net.obscura.vpnclientapp.ui.OsStatusManager

private val log = Logger(WebCmdBridge::class)

class WebCmdBridge(
    private val context: Context,
    private val binder: IObscuraVpnService,
    private val mainActivity: MainActivity,
    private val osStatusManager: OsStatusManager,
    private val postMessage: (data: String) -> Unit,
) {
    private val scope = CoroutineScope(Dispatchers.Main.immediate + SupervisorJob())

    @Serializable
    private data class Accept(
        val id: Long,
        val data: String,
    )

    @Serializable
    private data class Reject(
        val id: Long,
        val error: String,
    )

    private fun accept(id: Long, data: String) {
        this.postMessage(jsonConfig.encodeToString(Accept(id, data)))
    }

    private fun reject(id: Long, exception: ErrorCodeException) {
        this.postMessage(jsonConfig.encodeToString(Reject(id, exception.errorCode)))
    }

    @JavascriptInterface
    fun invoke(data: String, id: Long) {
        this.scope.launch {
            try {
                this@WebCmdBridge.accept(
                    id,
                    jsonConfig
                        .decodeFromString<WebCmd>(data)
                        .run(WebCmd.Args(context, binder, this@WebCmdBridge.mainActivity, osStatusManager)),
                )
            } catch (exception: CancellationException) {
                log.debug("invoke job canceled: ${exception.message}")
                throw exception
            } catch (exception: ErrorCodeException) {
                this@WebCmdBridge.reject(id, exception)
            } catch (exception: Throwable) {
                log.error("unexpected exception type: $exception", tr = exception)
                this@WebCmdBridge.reject(id, errorCodeOther())
            }
        }
    }

    fun cancel() {
        this.scope.cancel()
    }
}

package net.obscura.vpnclientapp.ui.bridge

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.webkit.JavascriptInterface
import java.lang.ref.WeakReference
import java.util.concurrent.CompletableFuture
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong
import java.util.function.BiFunction
import kotlinx.serialization.Serializable
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.ErrorCodeException
import net.obscura.vpnclientapp.client.errorCodeOther
import net.obscura.vpnclientapp.client.jsonConfig
import net.obscura.vpnclientapp.helpers.requireUIProcess
import net.obscura.vpnclientapp.helpers.requireVpnServiceProcess
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
    /**
     * This receiver receives the results from IObscuraVpnService.jsonFfi from the ObscuraVpnService running in a
     * separate :vpnservice process.
     */
    class Receiver : BroadcastReceiver() {
        companion object {
            private val waiting by lazy { ConcurrentHashMap<Long, CompletableFuture<String>>() }
            private val currentId = AtomicLong(System.currentTimeMillis())

            private const val EXTRA_ID = "id"
            private const val EXTRA_RESULT = "result"
            private const val EXTRA_EXCEPTION = "exception"

            fun register(fn: (id: Long) -> Unit): CompletableFuture<String> {
                requireUIProcess()

                val id = currentId.incrementAndGet()
                val future = CompletableFuture<String>()
                waiting.put(id, future)

                fn(id)

                return future
            }

            fun broadcast(context: Context, id: Long, future: CompletableFuture<String>) {
                requireVpnServiceProcess()

                future.handle { result, exception ->
                    context.sendBroadcast(
                        Intent(context, Receiver::class.java).apply {
                            putExtra(EXTRA_ID, id)

                            if (exception != null) {
                                putExtra(EXTRA_EXCEPTION, exception.message)
                            } else if (result != null) {
                                putExtra(EXTRA_RESULT, result)
                            }
                        }
                    )
                }
            }
        }

        override fun onReceive(context: Context, intent: Intent) {
            requireUIProcess()

            val id = intent.getLongExtra(EXTRA_ID, -1)
            val result = intent.getStringExtra(EXTRA_RESULT)
            val errorCode = intent.getStringExtra(EXTRA_EXCEPTION)

            log.debug("onReceive $id $result $errorCode")

            waiting.remove(id)?.let { future ->
                if (errorCode != null) {
                    future.completeExceptionally(ErrorCodeException(errorCode))
                } else if (result != null) {
                    future.complete(result)
                } else {
                    future.complete("null")
                }
            }
        }
    }

    private class Handler(val bridge: WeakReference<WebCmdBridge>, val id: Long) :
        BiFunction<String?, Throwable?, Unit> {
        override fun apply(data: String?, exception: Throwable?) {
            bridge.get()?.also { bridge ->
                if (exception != null) {
                    when (exception) {
                        is ErrorCodeException -> bridge.reject(exception, id)
                        else -> {
                            log.error("unexpected exception type: $exception", tr = exception)
                            bridge.reject(errorCodeOther(), id)
                        }
                    }
                } else if (data != null) {
                    bridge.accept(data, id)
                }
            }
        }
    }

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

    private fun accept(data: String, id: Long) {
        this.postMessage(jsonConfig.encodeToString(Accept(id, data)))
    }

    private fun reject(exception: ErrorCodeException, id: Long) {
        this.postMessage(jsonConfig.encodeToString(Reject(id, exception.errorCode)))
    }

    @JavascriptInterface
    fun invoke(data: String, id: Long) {
        jsonConfig
            .decodeFromString<WebCmd>(data)
            .run(WebCmd.Args(context, binder, this.mainActivity, osStatusManager))
            .handle(Handler(WeakReference(this), id))
    }
}

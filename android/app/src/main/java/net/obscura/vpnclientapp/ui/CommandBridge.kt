package net.obscura.vpnclientapp.ui

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
import kotlinx.serialization.json.Json
import net.obscura.vpnclientapp.client.JsonFfiException
import net.obscura.vpnclientapp.helpers.logDebug
import net.obscura.vpnclientapp.helpers.requireUIProcess
import net.obscura.vpnclientapp.helpers.requireVpnServiceProcess
import net.obscura.vpnclientapp.services.IObscuraVpnService
import net.obscura.vpnclientapp.ui.commands.InvokeCommand

class CommandBridge(
    val context: Context,
    val binder: IObscuraVpnService,
    val osStatus: OsStatus,
    val postMessage: (data: String) -> Unit,
) {
  /**
   * This receiver receives the results from IObscuraVpnService.jsonFfi from the ObscuraVpnService
   * running in a separate :vpnservice process.
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

      fun broadcast(
          context: Context,
          id: Long,
          future: CompletableFuture<String>,
      ) {
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
              },
          )
        }
      }
    }

    override fun onReceive(
        context: Context,
        intent: Intent,
    ) {
      requireUIProcess()

      val id = intent.getLongExtra(EXTRA_ID, -1)
      val result = intent.getStringExtra(EXTRA_RESULT)
      val exception = intent.getStringExtra(EXTRA_EXCEPTION)

      logDebug("onReceive $id $result $exception")

      waiting.remove(id)?.let { future ->
        if (exception != null) {
          future.completeExceptionally(JsonFfiException(exception))
        } else if (result != null) {
          future.complete(result)
        } else {
          future.complete("null")
        }
      }
    }
  }

  private class Handler(
      val bridge: WeakReference<CommandBridge>,
      val id: Long,
  ) : BiFunction<String?, Throwable?, Unit> {
    override fun apply(
        data: String?,
        exception: Throwable?,
    ) {
      bridge.get()?.also { bridge ->
        if (exception != null) {
          when (exception) {
            is JsonFfiException -> bridge.reject(exception.data, id)
            else -> throw exception // TODO: reject with error
          }
        } else if (data != null) {
          bridge.accept(data, id)
        }
      }
    }
  }

  @Serializable
  private data class AndroidCommandMessage(
      val id: Long,
      val error: String? = null,
      val data: String? = null,
  )

  val json = Json {
    encodeDefaults = true
    ignoreUnknownKeys = true
  }

  private fun accept(
      data: String,
      id: Long,
  ) {
    postMessage(
        Json.encodeToString(
            AndroidCommandMessage(
                id = id,
                data = data,
            ),
        ),
    )
  }

  private fun reject(
      data: String,
      id: Long,
  ) {
    postMessage(
        Json.encodeToString(
            AndroidCommandMessage(
                id = id,
                error = data,
            ),
        ),
    )
  }

  @JavascriptInterface
  fun invoke(
      data: String,
      id: Long,
  ) {
    val invokeData = json.decodeFromString<InvokeCommand>(data)

    invokeData.run(context, binder, osStatus, json).handle(Handler(WeakReference(this), id))
  }
}

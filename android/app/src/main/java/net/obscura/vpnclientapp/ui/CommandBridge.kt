package net.obscura.vpnclientapp.ui

import android.content.Context
import android.webkit.JavascriptInterface
import java.lang.ref.WeakReference
import java.util.function.BiFunction
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import net.obscura.vpnclientapp.client.JsonFfiException
import net.obscura.vpnclientapp.ui.commands.InvokeCommand

class CommandBridge(
    val context: Context,
    val postMessage: (data: String) -> Unit,
) {
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

    invokeData.run(context, json).handle(Handler(WeakReference(this), id))
  }
}

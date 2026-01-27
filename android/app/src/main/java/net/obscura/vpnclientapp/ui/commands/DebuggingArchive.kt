package net.obscura.vpnclientapp.ui.commands

import android.content.Context
import java.util.concurrent.CompletableFuture
import kotlinx.serialization.json.Json
import kotlinx.serialization.Serializable
import net.obscura.vpnclientapp.services.IObscuraVpnService
import net.obscura.vpnclientapp.ui.CommandBridge
import net.obscura.vpnclientapp.ui.OsStatus

@Serializable
data class DebuggingArchive(
    val userFeedback: String?,
) {
  @Serializable
  private data class CreateDebugArchive(
      val userFeedback: String?,
      val logParentDir: String,
  )

  // Eventually, all platforms should just use the JSON FFI command to create
  // debug archives, but for now, adapting the command here is the least
  // invasive change.
  // TODO: https://linear.app/soveng/issue/OBS-3095/cross-platform-debug-archive-story
  @Serializable
  private data class Cmd(
      val createDebugArchive: CreateDebugArchive,
  )

  fun run(context: Context, binder: IObscuraVpnService, osStatus: OsStatus, json: Json): CompletableFuture<String> {
    osStatus.debugBundleStatus.inProgress = true
    osStatus.update()
    val createDebugArchive = CreateDebugArchive(userFeedback, context.filesDir.absolutePath)
    val cmd = json.encodeToString(Cmd(createDebugArchive))
    return CommandBridge.Receiver
      .register { id -> binder.jsonFfi(id, cmd) }
      .whenComplete { result, throwable ->
        osStatus.debugBundleStatus.inProgress = false
        osStatus.debugBundleStatus.latestPath = json.decodeFromString(result)
        osStatus.update()
      }
  }
}

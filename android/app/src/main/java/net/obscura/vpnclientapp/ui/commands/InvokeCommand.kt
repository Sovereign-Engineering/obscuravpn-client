package net.obscura.vpnclientapp.ui.commands

import android.content.Context
import java.util.concurrent.CompletableFuture
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import net.obscura.vpnclientapp.helpers.completedJsonNullFuture
import net.obscura.vpnclientapp.services.IObscuraVpnService
import net.obscura.vpnclientapp.ui.CommandBridge
import net.obscura.vpnclientapp.ui.OsStatus

@Serializable
data class InvokeCommand(
    // TODO: https://linear.app/soveng/issue/OBS-2643/remove-timeoutms-from-commands-if-not-used Use
    // timeoutMs properly.
    val timeoutMs: Long? = null,
    val jsonFfiCmd: JsonFfiCommand? = null,
    val getOsStatus: GetOsStatus? = null,
    val emailArchive: EmailArchive? = null,
    val revealItemInDir: RevealItemInDir? = null,
    val setColorScheme: SetColorScheme? = null,
    val shareFile: ShareFile? = null,
    val startTunnel: StartTunnel? = null,
    val stopTunnel: JsonObject? = null,
) {
  fun run(
      context: Context,
      binder: IObscuraVpnService,
      osStatus: OsStatus,
      json: Json,
  ): CompletableFuture<String?> =
      when {
        getOsStatus != null -> getOsStatus.run(osStatus).thenApply { json.encodeToString(it) }

        jsonFfiCmd != null ->
            CommandBridge.Receiver.register { id -> binder.jsonFfi(id, jsonFfiCmd.cmd) }

        setColorScheme != null -> completedJsonNullFuture().also { setColorScheme.run(context) }

        shareFile != null -> shareFile.run(context).thenApply { "null" }

        emailArchive != null -> emailArchive.run(context).thenApply { "null" }

        revealItemInDir != null -> revealItemInDir.run(context).thenApply { "null" }

        startTunnel != null ->
            completedJsonNullFuture().also { binder.startTunnel(startTunnel.tunnelArgs) }

        stopTunnel != null -> completedJsonNullFuture().also { binder.stopTunnel() }

        else -> throw NotImplementedError("InvokeCommand not implemented")
      }
}

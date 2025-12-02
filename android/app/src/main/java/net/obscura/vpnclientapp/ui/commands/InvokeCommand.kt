package net.obscura.vpnclientapp.ui.commands

import android.content.Context
import java.util.concurrent.CompletableFuture
import kotlinx.serialization.Serializable
import kotlinx.serialization.encodeToString
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import net.obscura.vpnclientapp.client.ObscuraLibrary
import net.obscura.vpnclientapp.services.ObscuraVpnService

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
    val setStrictLeakPrevention: SetStrictLeakPrevention? = null,
    val shareFile: ShareFile? = null,
    val startTunnel: StartTunnel? = null,
    val stopTunnel: JsonObject? = null,
) {
  fun run(
      context: Context,
      json: Json,
  ): CompletableFuture<String> =
      when {
        getOsStatus != null -> getOsStatus.run(context).thenApply { json.encodeToString(it) }

        jsonFfiCmd != null ->
            CompletableFuture<String>().also { future ->
              ObscuraLibrary.jsonFfi(jsonFfiCmd.cmd, future)
            }

        setColorScheme != null ->
            CompletableFuture.completedFuture("null").also { setColorScheme.run(context) }

        setStrictLeakPrevention != null ->
            CompletableFuture.completedFuture("null").also { setStrictLeakPrevention.run(context) }

        shareFile != null -> shareFile.run(context).thenApply { "null" }

        emailArchive != null -> emailArchive.run(context).thenApply { "null" }

        revealItemInDir != null -> revealItemInDir.run(context).thenApply { "null" }

        startTunnel != null ->
            CompletableFuture.completedFuture("null").also {
              ObscuraVpnService.startTunnel(context, startTunnel.tunnelArgs!!)
            }

        stopTunnel != null ->
            CompletableFuture.completedFuture("null").also { ObscuraVpnService.stopTunnel(context) }

        else -> throw NotImplementedError("InvokeCommand not implemented")
      }
}

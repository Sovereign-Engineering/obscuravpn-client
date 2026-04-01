package net.obscura.vpnclientapp.ui.commands

import android.content.Context
import java.util.concurrent.CompletableFuture
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonObject
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.services.IObscuraVpnService
import net.obscura.vpnclientapp.ui.CommandBridge
import net.obscura.vpnclientapp.ui.OsStatus

private fun completedJsonNullFuture() = CompletableFuture.completedFuture("null")

@Serializable
data class InvokeCommand(
    val jsonFfiCmd: JsonFfiCommand? = null,
    val getOsStatus: GetOsStatus? = null,
    val debuggingArchive: DebuggingArchive? = null,
    val shareDebugArchive: ShareArchive? = null,
    val emailDebugArchive: EmailArchive? = null,
    val revealItemInDir: RevealItemInDir? = null,
    val purchaseSubscription: PurchaseSubscription? = null,
    val setColorScheme: SetColorScheme? = null,
    val startTunnel: StartTunnel? = null,
    val stopTunnel: JsonObject? = null,
) {
    fun run(
        context: Context,
        binder: IObscuraVpnService,
        mainActivity: MainActivity,
        osStatus: OsStatus,
        json: Json,
    ): CompletableFuture<String> =
        when {
            getOsStatus != null -> getOsStatus.run(osStatus).thenApply { json.encodeToString(it) }

            jsonFfiCmd != null -> CommandBridge.Receiver.register { id -> binder.jsonFfi(id, jsonFfiCmd.cmd) }

            purchaseSubscription != null -> purchaseSubscription.run(mainActivity)

            setColorScheme != null -> completedJsonNullFuture().also { setColorScheme.run(context) }

            debuggingArchive != null -> debuggingArchive.run(context, binder, osStatus, json)
            shareDebugArchive != null -> completedJsonNullFuture().also { shareDebugArchive.run(context) }
            emailDebugArchive != null -> completedJsonNullFuture().also { emailDebugArchive.run(context) }

            revealItemInDir != null -> revealItemInDir.run()

            startTunnel != null -> completedJsonNullFuture().also { binder.startTunnel(startTunnel.tunnelArgs) }

            stopTunnel != null -> completedJsonNullFuture().also { binder.stopTunnel() }

            else -> throw NotImplementedError("InvokeCommand not implemented")
        }
}

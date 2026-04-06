package net.obscura.vpnclientapp.ui.commands

import androidx.lifecycle.lifecycleScope
import java.util.concurrent.CompletableFuture
import kotlinx.coroutines.future.future
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.services.IObscuraVpnService

@Serializable
class StartTunnel(
    val tunnelArgs: String? = null,
) {
    fun run(binder: IObscuraVpnService, mainActivity: MainActivity): CompletableFuture<String> =
        mainActivity.lifecycleScope.future {
            mainActivity.vpnPermissionRequestManager
                .requestVpnStart()
                .getOrThrow()
                .let { Json.encodeToString(it) }
                .also { binder.startTunnel(this@StartTunnel.tunnelArgs) }
        }
}

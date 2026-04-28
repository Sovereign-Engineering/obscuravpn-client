package net.obscura.vpnclientapp.ui

import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.lifecycleScope
import kotlin.time.Duration.Companion.milliseconds
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.client.ManagerCmd
import net.obscura.vpnclientapp.client.ManagerCmdOk
import net.obscura.vpnclientapp.client.jsonConfig
import net.obscura.vpnclientapp.services.IObscuraVpnService

private val log = Logger(VpnStatusObserver::class)

class VpnStatusObserver(
    private val binder: IObscuraVpnService,
    private val callback: Callback,
) : DefaultLifecycleObserver {
    interface Callback {
        suspend fun onStatusChanged(status: ManagerCmdOk.GetStatus)
    }

    private var job: Job? = null

    override fun onStart(owner: LifecycleOwner) {
        this.job =
            owner.lifecycleScope.launch {
                var knownVersion: String? = null
                while (this.isActive) {
                    try {
                        val status =
                            JsonFfiBroadcastReceiver.waitForResponse(
                                    this@VpnStatusObserver.binder,
                                    jsonConfig.encodeToString(ManagerCmd.GetStatus(knownVersion)),
                                )
                                .await()
                                .let { jsonConfig.decodeFromString<ManagerCmdOk.GetStatus>(it) }
                        knownVersion = status.version
                        log.debug("updated VPN status: $status")
                        this@VpnStatusObserver.callback.onStatusChanged(status)
                    } catch (e: CancellationException) {
                        log.debug("VPN status job canceled: ${e.message}")
                        throw e
                    } catch (e: Throwable) {
                        log.error("failed to update VPN status: $e", tr = e)
                    }
                    delay(10.milliseconds)
                }
            }
    }

    override fun onStop(owner: LifecycleOwner) {
        this.job?.cancel(CancellationException("lifecycle owner stopped"))
    }

    override fun onDestroy(owner: LifecycleOwner) {
        this.job?.cancel(CancellationException("lifecycle owner destroyed"))
        owner.lifecycle.removeObserver(this)
    }
}

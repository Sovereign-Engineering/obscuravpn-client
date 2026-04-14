package net.obscura.vpnclientapp.ui

import android.content.Context
import dagger.hilt.android.qualifiers.ApplicationContext
import java.util.UUID
import javax.inject.Inject
import javax.inject.Singleton
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Deferred
import kotlinx.coroutines.completeWith
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.BuildConfig
import net.obscura.vpnclientapp.client.jsonConfig

private val log = Logger(OsStatusManager::class)

@Singleton // Prevents loss of state on activity destruction
class OsStatusManager @Inject constructor(@ApplicationContext context: Context) : NetworkStatusObserver.Callback {
    data class State(
        var debugBundleStatus: OsStatus.DebugBundleStatus =
            OsStatus.DebugBundleStatus(
                inProgress = false,
                latestPath = null,
                inProgressCounter = 0,
            ),
        var internetAvailable: Boolean = false,
        var vpnStatus: OsStatus.OsVpnStatus = OsStatus.OsVpnStatus.Disconnected,
    )

    private data class VersionedState(val version: UUID, val state: State)

    private var current = VersionedState(UUID.randomUUID(), State())
    private val waiting = ArrayList<CompletableDeferred<String>>()

    init {
        NetworkStatusObserver(context, this)
    }

    override fun onAvailableNetworksChanged(availableNetworks: Int) {
        this.update { this.internetAvailable = availableNetworks > 0 }
    }

    @Synchronized
    fun update(block: State.() -> Unit = {}) {
        val version = UUID.randomUUID()
        val result = runCatching {
            block(this.current.state)
            OsStatus(
                    version = version.toString(),
                    internetAvailable = this.current.state.internetAvailable,
                    osVpnStatus = this.current.state.vpnStatus,
                    srcVersion = BuildConfig.VERSION_NAME,
                    updaterStatus =
                        OsStatus.UpdaterStatus(
                            type = "uninitiated",
                            appcast = null,
                            error = null,
                            errorCode = null,
                        ),
                    debugBundleStatus = this.current.state.debugBundleStatus,
                    canSendMail = true,
                    loginItemStatus = null,
                    playBilling =
                        @Suppress("KotlinConstantConditions", "SimplifyBooleanWithConstants")
                        (BuildConfig.FLAVOR == "play"),
                )
                .let { jsonConfig.encodeToString(it) }
        }
        this.current = VersionedState(version, this.current.state)
        log.debug("updated OS status: ${this.current}")
        this.waiting.forEach { it.completeWith(result) }
        this.waiting.clear()
    }

    @Synchronized
    fun wait(knownVersion: String?): Deferred<String> =
        CompletableDeferred<String>().also {
            this.waiting.add(it)
            if (this.current.version.toString() != knownVersion) {
                this.update()
            }
        }
}

package net.obscura.vpnclientapp.ui

import android.content.Context
import android.content.SharedPreferences
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import java.util.UUID
import java.util.concurrent.CompletableFuture
import net.obscura.vpnclientapp.BuildConfig
import net.obscura.vpnclientapp.client.ManagerCmdOk
import net.obscura.vpnclientapp.helpers.requireUIProcess
import net.obscura.vpnclientapp.preferences.Preferences

class OsStatusManager(context: Context) {
    init {
        requireUIProcess()
    }

    private val preferences = Preferences(context)
    private val connectivityManager = context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager

    private val waiting = ArrayList<CompletableFuture<OsStatus>>()

    private var current: Pair<String, OsStatus>? = null

    private var vpnStatus: OsStatus.OsVpnStatus = OsStatus.OsVpnStatus.Disconnected

    fun setVpnStatus(vpnStatus: ManagerCmdOk.GetStatus.VpnStatus) {
        this.update {
            this.vpnStatus =
                when (vpnStatus) {
                    is ManagerCmdOk.GetStatus.VpnStatus.Connected -> OsStatus.OsVpnStatus.Connected
                    is ManagerCmdOk.GetStatus.VpnStatus.Connecting -> OsStatus.OsVpnStatus.Connecting
                    is ManagerCmdOk.GetStatus.VpnStatus.Disconnected -> OsStatus.OsVpnStatus.Disconnected
                }
        }
    }

    var debugBundleStatus: OsStatus.DebugBundleStatus =
        OsStatus.DebugBundleStatus(
            inProgress = false,
            latestPath = null,
            inProgressCounter = 0,
        )

    private val sharedPreferencesListener =
        SharedPreferences.OnSharedPreferenceChangeListener { _, key ->
            if (key == "strict-leak-prevention") {
                this.update()
            }
        }

    fun registerCallbacks() {
        this.preferences.registerListener(sharedPreferencesListener)
    }

    fun deregisterCallbacks() {
        this.preferences.unregisterListener(sharedPreferencesListener)
    }

    private fun hasInternet() =
        this.connectivityManager.activeNetwork?.let { network ->
            this.connectivityManager.getNetworkCapabilities(network)?.run {
                hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET) &&
                    hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED)
            } ?: false
        } ?: false

    fun update(block: OsStatusManager.() -> Unit = {}) {
        synchronized(this) {
            block(this)
            val status =
                OsStatus(
                    version = UUID.randomUUID().toString(),
                    internetAvailable = hasInternet(),
                    osVpnStatus = this.vpnStatus,
                    srcVersion = BuildConfig.VERSION_NAME,
                    updaterStatus =
                        OsStatus.UpdaterStatus(
                            type = "uninitiated",
                            appcast = null,
                            error = null,
                            errorCode = null,
                        ),
                    debugBundleStatus = this.debugBundleStatus,
                    canSendMail = true,
                    loginItemStatus = null,
                    playBilling =
                        @Suppress("KotlinConstantConditions", "SimplifyBooleanWithConstants")
                        (BuildConfig.FLAVOR == "play"),
                )
            this.current = Pair(status.version, status)
            this.waiting.forEach { it.complete(status) }
            this.waiting.clear()
        }
    }

    fun getStatus(knownVersion: String?): CompletableFuture<OsStatus> =
        synchronized(this) {
            CompletableFuture<OsStatus>().also {
                this.waiting.add(it)
                if (knownVersion == null || this.current == null || this.current?.first != knownVersion) {
                    this.update()
                }
            }
        }
}

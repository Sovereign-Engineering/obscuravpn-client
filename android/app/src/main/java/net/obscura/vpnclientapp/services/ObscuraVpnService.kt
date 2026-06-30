package net.obscura.vpnclientapp.services

import android.annotation.SuppressLint
import android.content.Intent
import android.content.pm.ServiceInfo
import android.net.ConnectivityManager
import android.net.ConnectivityManager.NetworkCallback
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.VpnService
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import android.os.ParcelFileDescriptor
import android.system.OsConstants
import java.net.NetworkInterface
import java.util.concurrent.CompletableFuture
import kotlin.concurrent.Volatile
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.BuildConfig
import net.obscura.vpnclientapp.client.ManagerCmd
import net.obscura.vpnclientapp.client.ManagerCmdOk
import net.obscura.vpnclientapp.client.RustFfi
import net.obscura.vpnclientapp.client.jsonConfig
import net.obscura.vpnclientapp.helpers.requireVpnServiceProcess
import net.obscura.vpnclientapp.ui.JsonFfiBroadcastReceiver

private val logNoFfi = Logger(ObscuraVpnService::class)

const val PATH_REQUEST_VPN_START = "requestVpnStart"

@SuppressLint("VpnServicePolicy")
class ObscuraVpnService : VpnService() {
    private class Binder(
        val service: ObscuraVpnService,
    ) : IObscuraVpnService.Stub() {
        override fun stopTunnel() {
            service.log.info("stopTunnel", "Gf6f2lwW")
            service.stopTunnel()
        }

        override fun jsonFfi(
            id: Long,
            command: String,
        ) {
            val future = CompletableFuture<String>()
            service.rustFfi.jsonFfi(command, future)
            future.handle { value: String?, exception: Throwable? ->
                try {
                    service.sendBroadcast(
                        Intent(service, JsonFfiBroadcastReceiver::class.java).apply {
                            this.putJsonFfiExtras(id, value, exception)
                        }
                    )
                } catch (e: Throwable) {
                    service.log.error("failed to broadcast job $id result: $e", messageId = "L74T4QBq", tr = e)
                }
            }
        }
    }

    companion object {
        private val instance = java.util.concurrent.atomic.AtomicReference<ObscuraVpnService?>(null)

        @androidx.annotation.Keep
        @JvmStatic
        fun ffiSetNetworkConfig(json: String): Int {
            val service = instance.get()
            if (service == null) {
                logNoFfi.error("ffiSetNetworkConfig called with no active service", "wK3xLm9p")
                return -1
            }
            val config: OsNetworkConfig =
                try {
                    jsonConfig.decodeFromString(json)
                } catch (e: Exception) {
                    service.log.error("failed to parse os network config: $e", "yN4zPn0q", e)
                    return -1
                }
            val pfd =
                try {
                    service.applyNetworkConfig(config)
                } catch (e: Exception) {
                    service.log.error("failed to apply os network config: $e", "U6hVQEJR", e)
                    return -1
                }
            return pfd?.detachFd() ?: -1
        }
    }

    private data class NetworkInterfaceProps(val name: String, val index: Int)

    private lateinit var rustFfi: RustFfi
    private lateinit var log: Logger
    private lateinit var notificationManager: NotificationManager
    private lateinit var handler: Handler

    private val connectivityManager
        get() = getSystemService(CONNECTIVITY_SERVICE) as ConnectivityManager

    @Volatile private var vpnStatus: ManagerCmdOk.GetStatus.VpnStatus? = null

    private var currentNetwork: Network? = null

    override fun onCreate() {
        super.onCreate()

        logNoFfi.info("ObscuraVpnService onCreate entry")
        rustFfi = RustFfi(this, "obscura.net/android/${BuildConfig.VERSION_NAME}")
        log = rustFfi.logger(logNoFfi.tag)

        if (instance.getAndSet(this) != null) {
            log.error("instance already initialized", "xR4mNb7c")
        }
        requireVpnServiceProcess()

        log.info("onCreate", "vqiGa01f")

        handler = Handler(Looper.getMainLooper())

        val networkRequest =
            NetworkRequest.Builder()
                .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
                .addCapability(NetworkCapabilities.NET_CAPABILITY_NOT_VPN)
                .build()
        val service = this
        connectivityManager.registerBestMatchingNetworkCallback(
            networkRequest,
            object : NetworkCallback() {
                override fun onAvailable(network: Network) {
                    service.currentNetwork = network
                    service.updateInterface(network)
                }

                override fun onLost(network: Network) {
                    if (network == service.currentNetwork) {
                        service.currentNetwork = null
                        service.updateInterface(null)
                    }
                }
            },
            handler,
        )

        this.notificationManager = NotificationManager(this)

        loadStatus(null)
    }

    private fun promoteToForeground(prepareResult: PrepareResult): Result<Unit> =
        runCatching {
                if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
                    // We only have permission to start a foreground service once our VPN profile has been created.
                    if (prepareResult == PrepareResult.Ready) {
                        this.startForeground(
                            NotificationManager.NOTIFICATION_ID,
                            this.notificationManager.buildNotification(this.vpnStatus, prepareResult),
                            ServiceInfo.FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED,
                        )
                    } else {
                        throw RuntimeException("VPN not prepared")
                    }
                } else {
                    this.startForeground(
                        NotificationManager.NOTIFICATION_ID,
                        this.notificationManager.buildNotification(this.vpnStatus, prepareResult),
                    )
                }
            }
            .onFailure {
                log.error(
                    "failed to promote service to foreground: ${it.message}",
                    "GKs2Deov",
                )
            }

    override fun onStartCommand(
        intent: Intent?,
        flags: Int,
        startId: Int,
    ): Int {
        log.info("onStartCommand $intent ${intent?.action} $flags $startId", "C9rsG0uh")
        val prepareResult = this.prepareVpnService()
        val isForeground = this.promoteToForeground(prepareResult).isSuccess
        when (intent?.action) {
            ACTION_START_TUNNEL -> if (isForeground) this.startTunnel(intent.getStartTunnelExtras())
            // `isForeground` is expected to always be true while disconnecting, but it's not a requirement.
            ACTION_STOP_TUNNEL -> this.stopTunnel()
            SERVICE_INTERFACE -> {
                log.info("onStartCommand was system-initiated", "sktWFegO")
                if (isForeground) this.startTunnel(null)
            }
        }
        if (!isForeground) {
            // This branch should only occur if the notification action is stale, which can happen if the user revokes
            // VPN permissions while disconnected.
            this.notificationManager.notify(this.vpnStatus, prepareResult)
            // If this was started using `startForegroundService`/`getForegroundService`, not calling `startForeground`
            // will still result in an ANR, but `START_STICKY` will recreate the service.
            this.stopSelfResult(startId)
        }
        return START_STICKY
    }

    override fun onBind(intent: Intent?): IBinder {
        log.info("onBind $intent ${intent?.action}", "lckBR8hX")
        if (intent?.action == SERVICE_INTERFACE) {
            log.info("onBind was system-initiated", "4olaayXf")
            this.promoteToForeground(PrepareResult.Ready)
        }
        // The default binder is what calls `onRevoke`, and will only be non-null if the action is `SERVICE_INTERFACE`.
        return super.onBind(intent) ?: Binder(this)
    }

    override fun onRebind(intent: Intent?) {
        log.info("onRebind $intent ${intent?.action}", "AcVtL2Ub")
        super.onRebind(intent)
        if (intent?.action == SERVICE_INTERFACE) {
            log.info("onRebind was system-initiated", "YsdxJ7Ni")
            this.promoteToForeground(PrepareResult.Ready)
        }
    }

    override fun onUnbind(intent: Intent?): Boolean {
        log.info("onUnbind $intent ${intent?.action}", "woAdA7g2")
        if (intent?.action == SERVICE_INTERFACE) {
            log.info("onUnbind was system-initiated", "oNOWQoPR")
            this.stopForeground(STOP_FOREGROUND_DETACH)
        }
        return true
    }

    private fun onStatusUpdated(status: ManagerCmdOk.GetStatus) {
        log.info("status updated $status", "xXx7PxdD")
        this.vpnStatus = status.vpnStatus
        this.notificationManager.notify(
            status.vpnStatus,
            if (status.vpnStatus == ManagerCmdOk.GetStatus.VpnStatus.Disconnected) this.prepareVpnService()
            else PrepareResult.Ready,
        )
        this.loadStatus(status.version)
    }

    // This will only be called if the system (`SERVICE_INTERFACE`) is bound to this service.
    // Unfortunately, the system unbinds after disconnecting the VPN, so this method will only be called if there's an
    // active connection when VPN permissions are revoked.
    override fun onRevoke() {
        log.info("onRevoke", "V3qS5kil")
        this.stopTunnel()
        super.onRevoke()
    }

    override fun onDestroy() {
        super.onDestroy()
        if (instance.getAndSet(null) == null) {
            log.error("instance already cleared", "bQ5wKr8d")
        }
        log.info("onDestroy", "yNLRpqaN")
        stopTunnel()
    }

    private fun loadStatus(knownVersion: String?) {
        log.info("load status $knownVersion", "8pXipD8h")

        CompletableFuture<String>().also {
            rustFfi.jsonFfi(
                jsonConfig.encodeToString(ManagerCmd.GetStatus(knownVersion)),
                it,
            )

            it.handle { data, tr ->
                log.info("getStatus completed $data", "oiAyY4gh", tr)
                data?.let { data -> onStatusUpdated(jsonConfig.decodeFromString(data)) }
            }
        }
    }

    private fun setTunnelArgs(exit: String?, active: Boolean?) {
        CompletableFuture<String>().also {
            rustFfi.jsonFfi(
                jsonConfig.encodeToString(
                    ManagerCmd.SetTunnelArgs(
                        args = exit?.let { exit -> jsonConfig.decodeFromString(exit) },
                        active,
                    ),
                ),
                it,
            )
        }
    }

    private fun stopTunnel() {
        setTunnelArgs(null, false)
    }

    private fun startTunnel(exitSelector: String?) {
        setTunnelArgs(exitSelector, true)
    }

    private fun applyNetworkConfig(networkConfig: OsNetworkConfig): ParcelFileDescriptor? {
        log.info("applying network config", "q9cnmRY0")

        val pfd =
            Builder()
                .apply {
                    // always disallow current app so it doesn't get routed through the VPN
                    addDisallowedApplication(applicationInfo.packageName)

                    setMtu(networkConfig.mtu)

                    // Inherit meteredness from the underlying network (set via setUnderlyingNetworks).
                    // Without this, VpnService.Builder defaults to always marking the VPN as metered,
                    // regardless of the underlying network.
                    setMetered(false)

                    // useSystemDns is always false on Android because no UI surface writes it.
                    //
                    // System-wide Private DNS overrides anything we set.
                    // Mullvad has the same problem, no fix without WRITE_SECURE_SETTINGS. UI warns about it.
                    // https://github.com/mullvad/mullvadvpn-app/issues/5009
                    if (!networkConfig.useSystemDns) {
                        networkConfig.dns.forEach { addDnsServer(it) }
                    }

                    networkConfig.ipv4.split("/").let { addAddress(it[0], if (it.size == 2) it[1].toInt() else 32) }

                    networkConfig.ipv6.split("/").let { addAddress(it[0], if (it.size == 2) it[1].toInt() else 128) }

                    networkConfig.routes.forEach { route -> addRoute(route.address, route.prefix) }

                    allowFamily(OsConstants.AF_INET)
                    allowFamily(OsConstants.AF_INET6)
                }
                .establish()

        if (pfd == null) {
            log.error("VpnService.Builder.establish() returned null", "tR7uWe2x")
        }
        return pfd
    }

    private fun getNetworkInterfaceProps(network: Network?): NetworkInterfaceProps? {
        val network = network ?: return null
        val linkProperties =
            this.connectivityManager.getLinkProperties(network)
                ?: run {
                    log.error("failed to get link properties for network: $network", "W0JKaOGP")
                    return null
                }
        val name =
            linkProperties.interfaceName
                ?: run {
                    log.error("network has no interface name: $network", "ukjpaGLl")
                    return null
                }
        val ni =
            NetworkInterface.getByName(name)
                ?: run {
                    log.error("failed to get interface by name: $name", "JvEt0GtR")
                    return null
                }
        log.info("setting network interface: $name ${ni.index}", "pOsKRATd")
        return NetworkInterfaceProps(name, ni.index)
    }

    private fun updateInterface(network: Network?) {
        log.info("network interface changed: $network", "crWriIOe")
        this.setUnderlyingNetworks(if (network != null) arrayOf(network) else emptyArray())
        val networkInterface = this.getNetworkInterfaceProps(network)
        if (networkInterface != null) {
            rustFfi.setNetworkInterface(networkInterface.name, networkInterface.index)
        } else {
            rustFfi.unsetNetworkInterface()
        }
    }
}

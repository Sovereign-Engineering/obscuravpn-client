package net.obscura.vpnclientapp.services

import android.Manifest
import android.annotation.SuppressLint
import android.app.PendingIntent
import android.content.Intent
import android.content.pm.PackageManager
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
import androidx.core.app.NotificationChannelCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import java.net.NetworkInterface
import java.util.concurrent.CompletableFuture
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.BuildConfig
import net.obscura.vpnclientapp.R
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.ManagerCmd
import net.obscura.vpnclientapp.client.ManagerCmdOk
import net.obscura.vpnclientapp.client.RustFfi
import net.obscura.vpnclientapp.client.jsonConfig
import net.obscura.vpnclientapp.helpers.requireVpnServiceProcess
import net.obscura.vpnclientapp.ui.JsonFfiBroadcastReceiver

private val logNoFfi = Logger(ObscuraVpnService::class)

@SuppressLint("VpnServicePolicy")
class ObscuraVpnService : VpnService() {
    private class Binder(
        val service: ObscuraVpnService,
    ) : IObscuraVpnService.Stub() {
        override fun startTunnel(exitSelector: String?) {
            service.log.info("startTunnel $exitSelector", "CddrThRg")
            service.startTunnel(exitSelector)
        }

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
        private const val NOTIFICATION_CHANNEL_ID = "vpn_channel"
        private const val NOTIFICATION_ID = 1

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
    private lateinit var handler: Handler

    private val connectivityManager
        get() = getSystemService(CONNECTIVITY_SERVICE) as ConnectivityManager

    private var vpnStatus: ManagerCmdOk.GetStatus.VpnStatus? = null

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

        createNotificationChannel()

        loadStatus(null)
    }

    private fun start() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            this.startForeground(
                NOTIFICATION_ID,
                this.buildNotification(),
                ServiceInfo.FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED,
            )
        } else {
            this.startForeground(NOTIFICATION_ID, this.buildNotification())
        }
    }

    override fun onStartCommand(
        intent: Intent?,
        flags: Int,
        startId: Int,
    ): Int {
        log.info("onStartCommand $intent ${intent?.action} $flags $startId", "C9rsG0uh")
        this.start()
        if (intent?.action == SERVICE_INTERFACE) {
            log.info("onStartCommand was system-initiated", "sktWFegO")
            this.startTunnel(null)
        }
        return START_STICKY
    }

    override fun onBind(intent: Intent?): IBinder {
        log.info("onBind $intent ${intent?.action}", "lckBR8hX")
        if (intent?.action == SERVICE_INTERFACE) {
            log.info("onBind was system-initiated", "4olaayXf")
            this.start()
        }
        return Binder(this)
    }

    override fun onRebind(intent: Intent?) {
        log.info("onRebind $intent ${intent?.action}", "AcVtL2Ub")
        super.onRebind(intent)
        if (intent?.action == SERVICE_INTERFACE) {
            log.info("onRebind was system-initiated", "YsdxJ7Ni")
            this.start()
        }
    }

    override fun onUnbind(intent: Intent?): Boolean {
        log.info("onUnbind $intent ${intent?.action}", "woAdA7g2")
        if (intent?.action == SERVICE_INTERFACE) {
            log.info("onUnbind was system-initiated", "oNOWQoPR")
            this.stopTunnel()
            this.stopForeground(STOP_FOREGROUND_DETACH)
        }
        return true
    }

    private fun onStatusUpdated(status: ManagerCmdOk.GetStatus) {
        log.info("status updated $status", "xXx7PxdD")
        vpnStatus = status.vpnStatus
        loadStatus(status.version)
        updateNotification()
    }

    override fun onRevoke() {
        super.onRevoke()
        log.info("onRevoke", "V3qS5kil")
        this.stopTunnel()
        this.stopForeground(STOP_FOREGROUND_DETACH)
    }

    override fun onDestroy() {
        super.onDestroy()
        if (instance.getAndSet(null) == null) {
            log.error("instance already cleared", "bQ5wKr8d")
        }
        log.info("onDestroy", "yNLRpqaN")
        stopTunnel()
    }

    private fun updateNotification() {
        // permission should already have been granted, but checking here to avoid crashes and to fix
        // the lint errors
        if (
            ContextCompat.checkSelfPermission(
                this,
                Manifest.permission.POST_NOTIFICATIONS,
            ) == PackageManager.PERMISSION_GRANTED
        ) {
            NotificationManagerCompat.from(this).notify(NOTIFICATION_ID, buildNotification())
        }
    }

    private fun buildNotification() =
        NotificationCompat.Builder(this, NOTIFICATION_CHANNEL_ID)
            .setContentIntent(
                PendingIntent.getActivity(
                    this,
                    0,
                    Intent().apply {
                        this.action = Intent.ACTION_MAIN
                        this.flags = Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP
                        this.setClassName(
                            BuildConfig.APPLICATION_ID,
                            MainActivity::class.qualifiedName!!,
                        )
                    },
                    PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
                )
            )
            .setContentTitle(getString(R.string.app_name))
            .setContentText(
                getString(
                    R.string.notification_vpn_text,
                    when (this.vpnStatus) {
                        is ManagerCmdOk.GetStatus.VpnStatus.Connected ->
                            getString(R.string.notification_vpn_status_connected)
                        is ManagerCmdOk.GetStatus.VpnStatus.Connecting ->
                            getString(R.string.notification_vpn_status_connecting)
                        is ManagerCmdOk.GetStatus.VpnStatus.Disconnected,
                        null -> getString(R.string.notification_vpn_status_disconnected)
                    },
                ),
            )
            .setSmallIcon(R.drawable.ic_stat_name)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
            .setOngoing(true)
            .setLocalOnly(true)
            .setOnlyAlertOnce(true)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .build()

    private fun createNotificationChannel() {
        NotificationManagerCompat.from(this)
            .createNotificationChannel(
                NotificationChannelCompat.Builder(
                        NOTIFICATION_CHANNEL_ID,
                        NotificationManagerCompat.IMPORTANCE_LOW,
                    )
                    .setName(getString(R.string.notification_channel_vpn_name))
                    .build(),
            )
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

                    if (!networkConfig.useSystemDns) {
                        networkConfig.dns.forEach { addDnsServer(it) }
                    }

                    networkConfig.ipv4.split("/").let { addAddress(it[0], if (it.size == 2) it[1].toInt() else 32) }

                    networkConfig.ipv6.split("/").let { addAddress(it[0], if (it.size == 2) it[1].toInt() else 128) }

                    addRoute("0.0.0.0", 0)
                    addRoute("::", 0)

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

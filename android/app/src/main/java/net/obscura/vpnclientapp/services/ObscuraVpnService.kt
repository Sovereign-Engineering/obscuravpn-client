package net.obscura.vpnclientapp.services

import android.Manifest
import android.annotation.SuppressLint
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
import kotlinx.serialization.json.Json
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.BuildConfig
import net.obscura.vpnclientapp.R
import net.obscura.vpnclientapp.client.RustFfi
import net.obscura.vpnclientapp.client.commands.GetStatus
import net.obscura.vpnclientapp.client.commands.SetTunnelArgs
import net.obscura.vpnclientapp.helpers.requireVpnServiceProcess
import net.obscura.vpnclientapp.ui.CommandBridge

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
            service.log.info("jsonFfi $id $command", "qMO4l3zd")
            CompletableFuture<String>().also {
                CommandBridge.Receiver.broadcast(service, id, it)
                service.rustFfi.jsonFfi(command, it)
            }
        }
    }

    companion object {
        private const val NOTIFICATION_CHANNEL_ID = "vpn_channel"
        private const val NOTIFICATION_ID = 1

        private val instance = java.util.concurrent.atomic.AtomicReference<ObscuraVpnService?>(null)

        @androidx.annotation.Keep
        @JvmStatic
        fun ffiSetNetworkConfig(json: String, context: Long) {
            val service = instance.get()
            if (service == null) {
                Logger(ObscuraVpnService::class).error("ffiSetNetworkConfig called with no active service", "wK3xLm9p")
                RustFfi.setNetworkConfigDone(context, -1)
                return
            }
            val config: OsNetworkConfig =
                try {
                    service.json.decodeFromString(json)
                } catch (e: Exception) {
                    service.log.error("failed to parse os network config: $e", "yN4zPn0q", e)
                    RustFfi.setNetworkConfigDone(context, -1)
                    return
                }
            val pfd =
                try {
                    service.applyNetworkConfig(config)
                } catch (e: Exception) {
                    service.log.error("failed to apply os network config: $e", "U6hVQEJR", e)
                    RustFfi.setNetworkConfigDone(context, -1)
                    return
                }
            if (pfd == null) {
                RustFfi.setNetworkConfigDone(context, -1)
            } else {
                RustFfi.setNetworkConfigDone(context, pfd.detachFd())
            }
        }
    }

    private data class NetworkInterfaceProps(val name: String, val index: Int)

    private lateinit var rustFfi: RustFfi
    private lateinit var log: Logger
    private lateinit var json: Json
    private lateinit var handler: Handler

    private val connectivityManager
        get() = getSystemService(CONNECTIVITY_SERVICE) as ConnectivityManager

    private var vpnStatus: GetStatus.Response.VpnStatus? = null

    private var currentNetwork: Network? = null

    override fun onCreate() {
        super.onCreate()

        Logger(ObscuraVpnService::class).info("ObscuraVpnService onCreate entry")
        rustFfi = RustFfi(this, "obscura.net/android/${BuildConfig.VERSION_NAME}")
        log = rustFfi.logger(ObscuraVpnService::class)

        if (instance.getAndSet(this) != null) {
            log.error("instance already initialized", "xR4mNb7c")
        }
        requireVpnServiceProcess()

        log.info("onCreate", "vqiGa01f")

        json = Json { ignoreUnknownKeys = true }
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

    override fun onStartCommand(
        intent: Intent?,
        flags: Int,
        startId: Int,
    ): Int {
        log.info("onStartCommand $intent $flags $startId", "C9rsG0uh")
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
            this.startForeground(
                NOTIFICATION_ID,
                this.buildNotification(),
                ServiceInfo.FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED,
            )
        } else {
            this.startForeground(NOTIFICATION_ID, this.buildNotification())
        }
        return START_STICKY
    }

    override fun onBind(intent: Intent?): IBinder {
        log.info("onBind $intent", "lckBR8hX")
        return Binder(this)
    }

    private fun onStatusUpdated(status: GetStatus.Response) {
        log.info("status updated $status", "xXx7PxdD")
        vpnStatus = status.vpnStatus
        loadStatus(status.version)
        updateNotification()
    }

    override fun onRevoke() {
        super.onRevoke()
        log.info("onRevoke", "V3qS5kil")
        stopTunnel()
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
            .setContentTitle(getString(R.string.app_name))
            .setContentText(
                getString(
                    R.string.notification_vpn_text,
                    vpnStatus.let {
                        when {
                            it?.connected != null -> getString(R.string.notification_vpn_status_connected)
                            it?.connecting != null -> getString(R.string.notification_vpn_status_connecting)

                            else -> getString(R.string.notification_vpn_status_disconnected)
                        }
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
                json.encodeToString(
                    GetStatus(GetStatus.Request(knownVersion = knownVersion)),
                ),
                it,
            )

            it.handle { data, tr ->
                log.info("getStatus completed $data", "oiAyY4gh", tr)
                data?.let { data -> onStatusUpdated(json.decodeFromString(data)) }
            }
        }
    }

    private fun setTunnelArgs(exit: String?, active: Boolean?) {
        CompletableFuture<String>().also {
            rustFfi.jsonFfi(
                json.encodeToString(
                    SetTunnelArgs(
                        SetTunnelArgs.Request(
                            args = exit?.let { exit -> json.decodeFromString(exit) },
                            active,
                        ),
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
                    networkConfig.dns?.forEach { addDnsServer(it) }

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

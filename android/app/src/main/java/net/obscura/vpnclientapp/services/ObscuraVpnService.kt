package net.obscura.vpnclientapp.services

import android.Manifest
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.content.pm.ServiceInfo
import android.net.ConnectivityManager
import android.net.ConnectivityManager.NetworkCallback
import android.net.LinkProperties
import android.net.Network
import android.net.NetworkCapabilities
import android.net.NetworkRequest
import android.net.VpnService
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.ParcelFileDescriptor
import android.system.OsConstants
import androidx.core.app.NotificationChannelCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import java.net.NetworkInterface
import java.util.concurrent.CompletableFuture
import kotlinx.serialization.json.Json
import net.obscura.vpnclientapp.R
import net.obscura.vpnclientapp.client.ObscuraLibrary
import net.obscura.vpnclientapp.client.commands.GetStatus
import net.obscura.vpnclientapp.client.commands.SetTunnelArgs
import net.obscura.vpnclientapp.helpers.currentApp
import net.obscura.vpnclientapp.helpers.debug
import net.obscura.vpnclientapp.ui.commands.GetOsStatus

class ObscuraVpnService : VpnService() {
  private class NetworkCallbackHandler(
      val service: ObscuraVpnService,
      val exitSelector: String,
  ) : NetworkCallback() {
    override fun onAvailable(network: Network) {
      super.onAvailable(network)

      debug("network is available $network")

      service.updateInterface(network)

      service.currentApp().osStatus.vpnStatus = GetOsStatus.Result.NEVPNStatus.Connected
      service.currentApp().osStatus.update()

      service.setTunnelArgs(exitSelector)
    }

    override fun onBlockedStatusChanged(
        network: Network,
        blocked: Boolean,
    ) {
      super.onBlockedStatusChanged(network, blocked)

      debug("network blocked status changed $network $blocked")

      service.updateInterface(network)
    }

    override fun onCapabilitiesChanged(
        network: Network,
        networkCapabilities: NetworkCapabilities,
    ) {
      super.onCapabilitiesChanged(network, networkCapabilities)

      debug("network capabilities changed $network $networkCapabilities")

      service.updateInterface(network)
    }

    override fun onLinkPropertiesChanged(
        network: Network,
        linkProperties: LinkProperties,
    ) {
      super.onLinkPropertiesChanged(network, linkProperties)

      debug("network link properties changed $network $linkProperties")

      service.updateInterface(network)
    }

    override fun onLosing(
        network: Network,
        maxMsToLive: Int,
    ) {
      super.onLosing(network, maxMsToLive)

      debug("loosing network $network $maxMsToLive")

      service.currentApp().osStatus.vpnStatus = GetOsStatus.Result.NEVPNStatus.Disconnecting
      service.currentApp().osStatus.update()
    }

    override fun onLost(network: Network) {
      super.onLost(network)

      debug("lost network $network")

      service.currentApp().osStatus.vpnStatus = GetOsStatus.Result.NEVPNStatus.Disconnected
      service.currentApp().osStatus.update()
    }
  }

  companion object {
    private val NOTIFICATION_CHANNEL_ID = "vpn_channel"
    private val NOTIFICATION_ID = 1

    private const val ACTION_START_TUNNEL = "start-tunnel"
    private const val ACTION_STOP_TUNNEL = "stop-tunnel"
    private const val EXTRA_EXIT_SELECTOR = "exit-selector"

    fun startTunnel(
        context: Context,
        exitSelector: String,
    ) {
      context.startForegroundService(
          Intent(context, ObscuraVpnService::class.java).apply {
            setAction(ACTION_START_TUNNEL)
            putExtra(EXTRA_EXIT_SELECTOR, exitSelector)
          },
      )
    }

    fun stopTunnel(context: Context) {
      context.startForegroundService(
          Intent(context, ObscuraVpnService::class.java).apply { setAction(ACTION_STOP_TUNNEL) },
      )
    }
  }

  private lateinit var json: Json
  private lateinit var handler: Handler

  private val connectivityManager
    get() = getSystemService(CONNECTIVITY_SERVICE) as ConnectivityManager

  private var networkCallbackHandler: NetworkCallbackHandler? = null

  private var vpnStatus: GetStatus.Response.VpnStatus? = null
  private var fd: ParcelFileDescriptor? = null

  override fun onCreate() {
    super.onCreate()

    debug("onCreate")

    json = Json { ignoreUnknownKeys = true }
    handler = Handler(Looper.getMainLooper())

    createNotificationChannel()

    loadStatus(null)
  }

  override fun onStartCommand(
      intent: Intent?,
      flags: Int,
      startId: Int,
  ): Int {
    debug("onStartCommand")

    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
      ServiceCompat.startForeground(
          this,
          NOTIFICATION_ID,
          buildNotification(),
          ServiceInfo.FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED,
      )
    } else {
      startForeground(NOTIFICATION_ID, buildNotification())
    }

    intent?.action?.let {
      when (it) {
        ACTION_START_TUNNEL -> startTunnel(intent.getStringExtra(EXTRA_EXIT_SELECTOR)!!)
        ACTION_STOP_TUNNEL -> stopTunnel()

        else -> throw RuntimeException("Unknown action $it")
      }
    }

    return START_STICKY
  }

  private fun onStatusUpdated(status: GetStatus.Response) {
    debug("status updated $status")

    loadStatus(status.version)

    vpnStatus = status.vpnStatus
    status.vpnStatus?.connected?.networkConfig?.let { networkConfig ->
      establishConnection(networkConfig)
    }

    updateNotification()
  }

  override fun onRevoke() {
    super.onRevoke()

    debug("onRevoke")

    stopTunnel()
  }

  override fun onDestroy() {
    super.onDestroy()

    debug("onDestroy")

    stopTunnel()
  }

  private fun updateNotification() {
    // permission should already have been granted, but checking here to avoid crashes and to fix
    // the lint errors
    if (ContextCompat.checkSelfPermission(
        this,
        Manifest.permission.POST_NOTIFICATIONS,
    ) == PackageManager.PERMISSION_GRANTED) {
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
                      it?.connecting != null ->
                          getString(R.string.notification_vpn_status_connecting)
                      else -> getString(R.string.notification_vpn_status_disconnected)
                    }
                  },
              ),
          )
          .setSmallIcon(R.drawable.ic_launcher_background)
          .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
          .setOngoing(true)
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
    debug("load status $knownVersion")

    CompletableFuture<String>().also {
      ObscuraLibrary.jsonFfi(
          json.encodeToString(
              GetStatus(GetStatus.Request(knownVersion = knownVersion)),
          ),
          it,
      )

      it.whenComplete { data, tr ->
        debug("getStatus completed $data", tr)

        data?.let { onStatusUpdated(json.decodeFromString(it)) }
      }
    }
  }

  private fun setTunnelArgs(exitSelector: String) {
    CompletableFuture<String>().also {
      ObscuraLibrary.jsonFfi(
          json.encodeToString(
              SetTunnelArgs(
                  SetTunnelArgs.Request(
                      args = json.decodeFromString(exitSelector),
                      allowActivation = true,
                  ),
              ),
          ),
          it,
      )

      it.whenComplete { data, tr -> debug("setTunnelArgs completed $data", tr) }
    }
  }

  private fun stopTunnel() {
    networkCallbackHandler?.let {
      connectivityManager.unregisterNetworkCallback(it)

      currentApp().osStatus.vpnStatus = GetOsStatus.Result.NEVPNStatus.Disconnected
      currentApp().osStatus.update()

      networkCallbackHandler = null
    }

    fd?.let { ObscuraLibrary.stopTunnel() }
    fd = null
  }

  private fun startTunnel(exitSelector: String) {
    stopTunnel()

    networkCallbackHandler =
        NetworkCallbackHandler(this, exitSelector).also {
          currentApp().osStatus.vpnStatus = GetOsStatus.Result.NEVPNStatus.Connecting
          currentApp().osStatus.update()

          connectivityManager.requestNetwork(
              NetworkRequest.Builder()
                  .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
                  .addCapability(NetworkCapabilities.NET_CAPABILITY_NOT_VPN)
                  .addCapability(NetworkCapabilities.NET_CAPABILITY_NOT_RESTRICTED)
                  .build(),
              it,
          )
        }
  }

  private fun establishConnection(
      networkConfig: GetStatus.Response.VpnStatus.Connected.NetworkConfig
  ) {
    fd?.let { ObscuraLibrary.stopTunnel() }

    fd =
        Builder()
            .apply {
              // always disallow current app so it doesn't get routed through the VPN
              addDisallowedApplication(applicationInfo.packageName)

              networkConfig.mtu?.let { setMtu(it) }
              networkConfig.dns?.forEach { it?.let({ dns -> addDnsServer(dns) }) }

              networkConfig.ipv4?.split("/")?.let {
                addAddress(
                    it[0],
                    if (it.size == 2) {
                      it[1].toInt()
                    } else {
                      32
                    },
                )
              }

              networkConfig.ipv6?.split("/")?.let {
                addAddress(
                    it[0],
                    if (it.size == 2) {
                      it[1].toInt()
                    } else {
                      128
                    },
                )
              }

              addRoute("0.0.0.0", 0)
              addRoute("::", 0)

              allowFamily(OsConstants.AF_INET)
              allowFamily(OsConstants.AF_INET6)
            }
            .establish()
            ?.apply {
              ObscuraLibrary.startTunnel(detachFd())

              debug("VPN tunnel started")
            }
  }

  private fun updateInterface(network: Network?) {
    if (network != null) {
      val interfaceName = connectivityManager.getLinkProperties(network)?.interfaceName

      if (interfaceName != null) {
        val netInterface = NetworkInterface.getByName(interfaceName)

        if (netInterface != null) {
          ObscuraLibrary.setNetworkInterfaceIndex(netInterface.index)

          setUnderlyingNetworks(arrayOf(network))
        }
      }
    }
  }
}

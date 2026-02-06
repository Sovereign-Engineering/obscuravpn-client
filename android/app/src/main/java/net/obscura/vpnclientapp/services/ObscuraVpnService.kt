package net.obscura.vpnclientapp.services

import android.Manifest
import android.annotation.SuppressLint
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
import net.obscura.vpnclientapp.R
import net.obscura.vpnclientapp.client.ObscuraLibrary
import net.obscura.vpnclientapp.client.commands.GetStatus
import net.obscura.vpnclientapp.client.commands.SetTunnelArgs
import net.obscura.vpnclientapp.helpers.logInfo
import net.obscura.vpnclientapp.helpers.logError
import net.obscura.vpnclientapp.helpers.requireVpnServiceProcess
import net.obscura.vpnclientapp.ui.CommandBridge
import net.obscura.vpnclientapp.ui.OsStatus
import net.obscura.vpnclientapp.ui.commands.GetOsStatus

@SuppressLint("VpnServicePolicy")
class ObscuraVpnService : VpnService() {
  private class NetworkCallbackHandler(
      val service: ObscuraVpnService,
      val exitSelector: String?,
  ) : NetworkCallback() {
    override fun onAvailable(network: Network) {
      super.onAvailable(network)

      logInfo("network is available $network", "sjIGwIBY")
      if (service.currentNetwork != null) {
        service.setUnderlyingNetworks(arrayOf(service.currentNetwork))
      } else {
        service.setUnderlyingNetworks(emptyArray())
      }
      service.setTunnelArgs(exitSelector, true)
      service.updateNEVPNStatus(GetOsStatus.Result.NEVPNStatus.Connected)
    }

    override fun onBlockedStatusChanged(
        network: Network,
        blocked: Boolean,
    ) {
      super.onBlockedStatusChanged(network, blocked)

      logInfo("network blocked status changed $network $blocked", "dVomhtV1")

      service.updateInterface(network)
      service.updateNEVPNStatus(GetOsStatus.Result.NEVPNStatus.Connected)
    }

    override fun onCapabilitiesChanged(
        network: Network,
        networkCapabilities: NetworkCapabilities,
    ) {
      super.onCapabilitiesChanged(network, networkCapabilities)

      logInfo("network capabilities changed $network $networkCapabilities", "APRCQ1hd")

      service.updateInterface(network)
      service.updateNEVPNStatus(GetOsStatus.Result.NEVPNStatus.Connected)
    }

    override fun onLinkPropertiesChanged(
        network: Network,
        linkProperties: LinkProperties,
    ) {
      super.onLinkPropertiesChanged(network, linkProperties)

      logInfo("network link properties changed $network $linkProperties", "GF2XfMPW")

      service.updateInterface(network)
      service.updateNEVPNStatus(GetOsStatus.Result.NEVPNStatus.Connected)
    }

    override fun onLosing(
        network: Network,
        maxMsToLive: Int,
    ) {
      super.onLosing(network, maxMsToLive)

      logInfo("loosing network $network $maxMsToLive", "Q23Uvo5K")

      service.updateNEVPNStatus(GetOsStatus.Result.NEVPNStatus.Disconnecting)
    }

    override fun onLost(network: Network) {
      super.onLost(network)

      logInfo("lost network $network", "zOCU8MXj")

      service.updateNEVPNStatus(GetOsStatus.Result.NEVPNStatus.Disconnected)
    }
  }

  private class Binder(
      val service: ObscuraVpnService,
  ) : IObscuraVpnService.Stub() {
    override fun startTunnel(exitSelector: String?) {
      logInfo("startTunnel $exitSelector", "CddrThRg")

      service.startTunnel(exitSelector)
    }

    override fun stopTunnel() {
      logInfo("stopTunnel", "Gf6f2lwW")

      service.stopTunnel()
    }

    override fun jsonFfi(
        id: Long,
        command: String?,
    ) {
      logInfo("jsonFfi $id $command", "qMO4l3zd")

      CompletableFuture<String>().also {
        CommandBridge.Receiver.broadcast(service, id, it)
        ObscuraLibrary.jsonFfi(command!!, it)
      }
    }
  }

  companion object {
    private const val NOTIFICATION_CHANNEL_ID = "vpn_channel"
    private const val NOTIFICATION_ID = 1
  }

  private lateinit var json: Json
  private lateinit var handler: Handler

  private val connectivityManager
    get() = getSystemService(CONNECTIVITY_SERVICE) as ConnectivityManager

  private var networkCallbackHandler: NetworkCallbackHandler? = null

  private var vpnStatus: GetStatus.Response.VpnStatus? = null

  private var neVpnStatus: GetOsStatus.Result.NEVPNStatus =
      GetOsStatus.Result.NEVPNStatus.Disconnected

  private var fd: ParcelFileDescriptor? = null
  private var currentNetwork: Network? = null

  override fun onCreate() {
    super.onCreate()

    requireVpnServiceProcess()

    logInfo("onCreate", "vqiGa01f")

    json = Json { ignoreUnknownKeys = true }
    handler = Handler(Looper.getMainLooper())

      val networkRequest = NetworkRequest.Builder()
          .addCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET)
          .addCapability(NetworkCapabilities.NET_CAPABILITY_NOT_VPN)
          .build()
      val service = this
      connectivityManager.registerBestMatchingNetworkCallback(networkRequest, object : NetworkCallback() {
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
      }, handler)

    createNotificationChannel()

    loadStatus(null)
  }

  override fun onStartCommand(
      intent: Intent?,
      flags: Int,
      startId: Int,
  ): Int {
    logInfo("onStartCommand $intent $flags $startId", "C9rsG0uh")
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE) {
      this.startForeground(
          NOTIFICATION_ID,
          this.buildNotification(),
          ServiceInfo.FOREGROUND_SERVICE_TYPE_SYSTEM_EXEMPTED,
      )
    } else {
      this.startForeground(NOTIFICATION_ID, this.buildNotification())
    }
    this.updateNEVPNStatus(neVpnStatus)
    return START_STICKY
  }

  override fun onBind(intent: Intent?): IBinder? {
    logInfo("onBind $intent", "lckBR8hX")

    updateNEVPNStatus(neVpnStatus)

    return Binder(this)
  }

  private fun onStatusUpdated(status: GetStatus.Response) {
    logInfo("status updated $status", "xXx7PxdD")

    loadStatus(status.version)

    vpnStatus = status.vpnStatus
    status.vpnStatus?.connected?.networkConfig?.let { networkConfig ->
      establishConnection(networkConfig)
    }

    updateNotification()
  }

  override fun onRevoke() {
    super.onRevoke()

    logInfo("onRevoke", "V3qS5kil")

    stopTunnel()
  }

  override fun onDestroy() {
    super.onDestroy()

    logInfo("onDestroy", "yNLRpqaN")

    stopTunnel()
  }

  private fun updateNEVPNStatus(status: GetOsStatus.Result.NEVPNStatus) {
    neVpnStatus = status
    OsStatus.Receiver.broadcast(this, status)
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
    logInfo("load status $knownVersion", "8pXipD8h")

    CompletableFuture<String>().also {
      ObscuraLibrary.jsonFfi(
          json.encodeToString(
              GetStatus(GetStatus.Request(knownVersion = knownVersion)),
          ),
          it,
      )

      it.handle { data, tr ->
        logInfo("getStatus completed $data", "oiAyY4gh", tr)

        data?.let { data -> onStatusUpdated(json.decodeFromString(data)) }
      }
    }
  }

  private fun setTunnelArgs(exit: String?, active: Boolean?) {
    CompletableFuture<String>().also {
      ObscuraLibrary.jsonFfi(
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
    networkCallbackHandler?.let {
      updateNEVPNStatus(GetOsStatus.Result.NEVPNStatus.Disconnecting)

      connectivityManager.unregisterNetworkCallback(it)

      updateNEVPNStatus(GetOsStatus.Result.NEVPNStatus.Disconnected)

      networkCallbackHandler = null
    }

    fd?.let {
      ObscuraLibrary.stopTunnel()
      setTunnelArgs(null, false)
    }
    fd = null
  }

  private fun startTunnel(exitSelector: String?) {
    stopTunnel()

    OsStatus.Receiver.broadcast(this, GetOsStatus.Result.NEVPNStatus.Connecting)

    networkCallbackHandler =
        NetworkCallbackHandler(this, exitSelector).also {
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
              networkConfig.dns?.forEach { it?.let { dns -> addDnsServer(dns) } }

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

              logInfo("VPN tunnel started", "q9cnmRY1")
            }
  }

    private fun updateInterface(network: Network?) {
        logInfo("network interface changed: $network", "crWriIOe")
        if (network != null) {
            setUnderlyingNetworks(arrayOf(network))
        } else {
            setUnderlyingNetworks(emptyArray())
        }
        val ifIndex: Int? = if (network != null) {
            val linkProperties = connectivityManager.getLinkProperties(network)
            if (linkProperties == null) {
                logError("failed to get link properties", "W0JKaOGP")
                null
            } else {
                val ifName = linkProperties.interfaceName
                if (ifName == null) {
                    logError("interface name is not set", "ukjpaGLl")
                    null
                } else {
                    val ifIndex = NetworkInterface.getByName(ifName)?.index
                    if (ifIndex == null) {
                        logError("interface lookup by name $ifName failed", "JvEt0GtR")
                        null
                    } else {
                        ifIndex
                    }
                }
            }
        } else {
            null
        }
        logInfo("setting interface index $ifIndex", "pOsKRATd")
        ObscuraLibrary.setNetworkInterfaceIndex(ifIndex ?: 0)
    }
}

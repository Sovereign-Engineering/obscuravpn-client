package net.obscura.vpnclientapp.services

import android.Manifest
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
import androidx.core.app.ServiceCompat
import androidx.core.content.ContextCompat
import java.net.NetworkInterface
import java.util.concurrent.CompletableFuture
import kotlinx.serialization.json.Json
import net.obscura.vpnclientapp.R
import net.obscura.vpnclientapp.client.ObscuraLibrary
import net.obscura.vpnclientapp.client.commands.GetStatus
import net.obscura.vpnclientapp.client.commands.SetTunnelArgs
import net.obscura.vpnclientapp.helpers.debug
import net.obscura.vpnclientapp.helpers.requireVpnServiceProcess
import net.obscura.vpnclientapp.ui.CommandBridge
import net.obscura.vpnclientapp.ui.OsStatus
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
      service.setTunnelArgs(exitSelector)
      service.updateNEVPNStatus(GetOsStatus.Result.NEVPNStatus.Connected)
    }

    override fun onBlockedStatusChanged(
        network: Network,
        blocked: Boolean,
    ) {
      super.onBlockedStatusChanged(network, blocked)

      debug("network blocked status changed $network $blocked")

      service.updateInterface(network)
      service.updateNEVPNStatus(GetOsStatus.Result.NEVPNStatus.Connected)
    }

    override fun onCapabilitiesChanged(
        network: Network,
        networkCapabilities: NetworkCapabilities,
    ) {
      super.onCapabilitiesChanged(network, networkCapabilities)

      debug("network capabilities changed $network $networkCapabilities")

      service.updateInterface(network)
      service.updateNEVPNStatus(GetOsStatus.Result.NEVPNStatus.Connected)
    }

    override fun onLinkPropertiesChanged(
        network: Network,
        linkProperties: LinkProperties,
    ) {
      super.onLinkPropertiesChanged(network, linkProperties)

      debug("network link properties changed $network $linkProperties")

      service.updateInterface(network)
      service.updateNEVPNStatus(GetOsStatus.Result.NEVPNStatus.Connected)
    }

    override fun onLosing(
        network: Network,
        maxMsToLive: Int,
    ) {
      super.onLosing(network, maxMsToLive)

      debug("loosing network $network $maxMsToLive")

      service.updateNEVPNStatus(GetOsStatus.Result.NEVPNStatus.Disconnecting)
    }

    override fun onLost(network: Network) {
      super.onLost(network)

      debug("lost network $network")

      service.updateNEVPNStatus(GetOsStatus.Result.NEVPNStatus.Disconnected)
    }
  }

  private class Binder(
      val service: ObscuraVpnService,
  ) : IObscuraVpnService.Stub() {
    override fun startTunnel(exitSelector: String?) {
      debug("startTunnel $exitSelector")

      service.startTunnel(exitSelector!!)
    }

    override fun stopTunnel() {
      debug("stopTunnel")

      service.stopTunnel()
    }

    override fun jsonFfi(
        id: Long,
        command: String?,
    ) {
      debug("jsonFfi $id $command")

      CompletableFuture<String>().also {
        CommandBridge.Receiver.broadcast(service, id, it)
        ObscuraLibrary.jsonFfi(command!!, it)
      }
    }
  }

  companion object {
    private val NOTIFICATION_CHANNEL_ID = "vpn_channel"
    private val NOTIFICATION_ID = 1
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

  override fun onCreate() {
    super.onCreate()

    requireVpnServiceProcess()

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

    updateNEVPNStatus(neVpnStatus)

    return START_STICKY
  }

  override fun onBind(intent: Intent?): IBinder? {
    debug("onBind $intent")

    updateNEVPNStatus(neVpnStatus)

    return Binder(this)
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
          .setSmallIcon(R.drawable.ic_launcher_background)
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
    debug("load status $knownVersion")

    CompletableFuture<String>().also {
      ObscuraLibrary.jsonFfi(
          json.encodeToString(
              GetStatus(GetStatus.Request(knownVersion = knownVersion)),
          ),
          it,
      )

      it.handle { data, tr ->
        debug("getStatus completed $data", tr)

        data?.let { onStatusUpdated(json.decodeFromString(it)) }
      }
    }
  }

  private fun setTunnelArgs(exitSelector: String?) {
    CompletableFuture<String>().also {
      ObscuraLibrary.jsonFfi(
          json.encodeToString(
              SetTunnelArgs(
                  SetTunnelArgs.Request(
                      args =
                          if (exitSelector == null) {
                            null
                          } else {
                            json.decodeFromString(exitSelector)
                          },
                      allowActivation = exitSelector != null,
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
      setTunnelArgs(null)
    }
    fd = null
  }

  private fun startTunnel(exitSelector: String) {
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

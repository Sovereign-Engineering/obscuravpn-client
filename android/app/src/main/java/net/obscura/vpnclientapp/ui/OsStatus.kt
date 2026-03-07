package net.obscura.vpnclientapp.ui

import android.content.Context
import android.content.SharedPreferences
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import java.util.UUID
import java.util.concurrent.CompletableFuture
import net.obscura.vpnclientapp.BuildConfig
import net.obscura.vpnclientapp.client.commands.GetStatus
import net.obscura.vpnclientapp.helpers.requireUIProcess
import net.obscura.vpnclientapp.preferences.Preferences
import net.obscura.vpnclientapp.ui.commands.GetOsStatus

class OsStatus(
    context: Context,
) {
  init {
    requireUIProcess()
  }

  private val preferences = Preferences(context)
  private val connectivityManager =
      context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager

  private val waiting = ArrayList<CompletableFuture<GetOsStatus.Result>>()

  private var current: Pair<String, GetOsStatus.Result>? = null

  private var vpnStatus: GetOsStatus.Result.OsVpnStatus = GetOsStatus.Result.OsVpnStatus.Disconnected

  fun setVpnStatus(vpnStatus: GetStatus.Response.VpnStatus) {
    synchronized(this) {
      this.vpnStatus = when {
        vpnStatus.connected != null -> GetOsStatus.Result.OsVpnStatus.Connected
        vpnStatus.connecting != null -> GetOsStatus.Result.OsVpnStatus.Connecting
        else -> GetOsStatus.Result.OsVpnStatus.Disconnected
      }
      update()
    }
  }

  var debugBundleStatus: GetOsStatus.Result.DebugBundleStatus = GetOsStatus.Result.DebugBundleStatus(
      inProgress = false,
      latestPath = null,
      inProgressCounter = 0,
  )

  private val sharedPreferencesListener =
      SharedPreferences.OnSharedPreferenceChangeListener { sharedPreferences, key ->
        if (key == "strict-leak-prevention") {
          update()
        }
      }

  fun registerCallbacks() {
    preferences.registerListener(sharedPreferencesListener)
  }

  fun deregisterCallbacks() {
    preferences.unregisterListener(sharedPreferencesListener)
  }

  private fun hasInternet() =
      connectivityManager.activeNetwork?.let { network ->
        connectivityManager.getNetworkCapabilities(network)?.run {
          hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET) &&
              hasCapability(NetworkCapabilities.NET_CAPABILITY_VALIDATED)
        } ?: false
      } ?: false

  fun update() {
    synchronized(this) {
      val version = UUID.randomUUID().toString()
      val result =
          GetOsStatus.Result(
              version = version,
              internetAvailable = hasInternet(),
              osVpnStatus = vpnStatus,
              srcVersion = BuildConfig.VERSION_NAME,
              updaterStatus =
                  GetOsStatus.Result.UpdaterStatus(
                      type = "uninitiated",
                      appcast = null,
                      error = null,
                      errorCode = null,
                  ),
              debugBundleStatus,
              canSendMail = true,
              loginItemStatus = null,
          )

      current = Pair(version, result)

      waiting.forEach { it.complete(result) }
      waiting.clear()
    }
  }

  fun getStatus(knownVersion: String?): CompletableFuture<GetOsStatus.Result> =
      synchronized(this) {
        CompletableFuture<GetOsStatus.Result>().also {
          waiting.add(it)

          if (knownVersion == null || current == null || current?.first != knownVersion) {
            update()
          }
        }
      }
}

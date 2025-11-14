package net.obscura.vpnclientapp.ui

import android.content.Context
import android.content.SharedPreferences
import android.net.ConnectivityManager
import android.net.Network
import android.net.NetworkCapabilities
import java.util.UUID
import java.util.concurrent.CompletableFuture
import net.obscura.vpnclientapp.preferences.Preferences
import net.obscura.vpnclientapp.ui.commands.GetOsStatus

class OsStatus(
    context: Context,
) {
  private val preferences = Preferences(context)
  private val connectivityManager =
      context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager

  private val waiting = ArrayList<CompletableFuture<GetOsStatus.Result>>()

  private var current: Pair<String, GetOsStatus.Result>? = null

  private val networkCallback =
      object : ConnectivityManager.NetworkCallback() {
        override fun onCapabilitiesChanged(
            network: Network,
            networkCapabilities: NetworkCapabilities,
        ) {
          super.onCapabilitiesChanged(network, networkCapabilities)

          update()
        }

        override fun onLost(network: Network) {
          super.onLost(network)

          update()
        }
      }

  private val sharedPreferencesListener =
      SharedPreferences.OnSharedPreferenceChangeListener { sharedPreferences, key ->
        if (key == "strict-leak-prevention") {
          update()
        }
      }

  fun registerCallbacks() {
    connectivityManager.registerDefaultNetworkCallback(networkCallback)
    preferences.registerListener(sharedPreferencesListener)
  }

  fun deregisterCallbacks() {
    connectivityManager.unregisterNetworkCallback(networkCallback)
    preferences.unregisterListener(sharedPreferencesListener)
  }

  // TODO:
  // https://linear.app/soveng/issue/OBS-2641/investigate-need-for-has-internet-boolean-in-getosstatus Looks like this is only needed for iOS?
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
              osVpnStatus = GetOsStatus.Result.NEVPNStatus.Invalid,
              srcVersion = "TODO",
              strictLeakPrevention = preferences.strictLeakPrevention,
              updaterStatus =
                  GetOsStatus.Result.UpdaterStatus(
                      type = "initiated",
                      appcast = null,
                      error = null,
                      errorCode = null,
                  ),
              debugBundleStatus =
                  GetOsStatus.Result.DebugBundleStatus(
                      inProgress = false,
                      latestPath = null,
                      inProgressCounter = 0,
                  ),
              canSendMail = false,
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

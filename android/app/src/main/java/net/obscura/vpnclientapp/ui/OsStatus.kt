package net.obscura.vpnclientapp.ui

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.SharedPreferences
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import java.lang.ref.WeakReference
import java.util.LinkedList
import java.util.UUID
import java.util.concurrent.CompletableFuture
import net.obscura.vpnclientapp.BuildConfig
import net.obscura.vpnclientapp.helpers.requireUIProcess
import net.obscura.vpnclientapp.helpers.requireVpnServiceProcess
import net.obscura.vpnclientapp.preferences.Preferences
import net.obscura.vpnclientapp.ui.commands.GetOsStatus

class OsStatus(
    context: Context,
) {
  class Receiver : BroadcastReceiver() {
    companion object {
      private const val EXTRA_STATUS = "status"

      internal val osStatuses = LinkedList<WeakReference<OsStatus>>()

      fun broadcast(
          context: Context,
          status: GetOsStatus.Result.NEVPNStatus,
      ) {
        requireVpnServiceProcess()

        context.sendOrderedBroadcast(
            Intent(context, Receiver::class.java).apply { putExtra(EXTRA_STATUS, status.name) },
            null,
        )
      }
    }

    override fun onReceive(
        context: Context?,
        intent: Intent,
    ) {
      requireUIProcess()

      val vpnStatus = GetOsStatus.Result.NEVPNStatus.valueOf(intent.getStringExtra(EXTRA_STATUS)!!)

      synchronized(Receiver) {
        osStatuses.listIterator().apply {
          while (hasNext()) {
            val osStatus = next().get()

            if (osStatus == null) {
              remove()
            } else {
              synchronized(osStatus) {
                osStatus.vpnStatus = vpnStatus
                osStatus.update()
              }
            }
          }
        }
      }
    }
  }

  init {
    // This object is not to be constructed in the ObscuraVpnService process space.
    requireUIProcess()

    synchronized(Receiver) { Receiver.osStatuses.add(WeakReference(this)) }
  }

  private val preferences = Preferences(context)
  private val connectivityManager =
      context.getSystemService(Context.CONNECTIVITY_SERVICE) as ConnectivityManager

  private val waiting = ArrayList<CompletableFuture<GetOsStatus.Result>>()

  private var current: Pair<String, GetOsStatus.Result>? = null

  private var vpnStatus: GetOsStatus.Result.NEVPNStatus =
      GetOsStatus.Result.NEVPNStatus.Disconnected

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
              osVpnStatus = vpnStatus,
              srcVersion = BuildConfig.VERSION_NAME,
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

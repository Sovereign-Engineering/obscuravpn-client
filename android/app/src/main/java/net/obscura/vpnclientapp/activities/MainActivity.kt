package net.obscura.vpnclientapp.activities

import android.Manifest
import android.content.ComponentName
import android.content.Intent
import android.content.ServiceConnection
import android.content.SharedPreferences
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.net.VpnService
import android.os.Build
import android.os.Bundle
import android.os.IBinder
import androidx.activity.addCallback
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.app.AppCompatDelegate
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import net.obscura.vpnclientapp.R
import net.obscura.vpnclientapp.helpers.logDebug
import net.obscura.vpnclientapp.helpers.requireUIProcess
import net.obscura.vpnclientapp.preferences.Preferences
import net.obscura.vpnclientapp.services.IObscuraVpnService
import net.obscura.vpnclientapp.services.ObscuraVpnService
import net.obscura.vpnclientapp.ui.ObscuraUI
import net.obscura.vpnclientapp.ui.OsStatus
import net.obscura.vpnclientapp.ui.commands.SetColorScheme

class MainActivity : AppCompatActivity(), SharedPreferences.OnSharedPreferenceChangeListener {
  enum class RequestCodes(
      val code: Int,
  ) {
    VPN_PREPARE(111111),
    NOTIFICATIONS(222222),
  }

  private class VpnServiceConnection(
      val activity: MainActivity,
  ) : ServiceConnection {
    override fun onServiceConnected(
        name: ComponentName?,
        service: IBinder?,
    ) {
      logDebug("onServiceConnected $name $service")

      activity.ui.onCreate(IObscuraVpnService.Stub.asInterface(service), activity.osStatus)
    }

    override fun onServiceDisconnected(name: ComponentName?) {
      logDebug("onServiceDisconnected $name")

      activity.ui.onDestroy()

      if (activity.vpnServiceConnection === this) {
        activity.vpnServiceConnection = null
      }
    }
  }

  private lateinit var preferences: Preferences
  private lateinit var osStatus: OsStatus

  private lateinit var ui: ObscuraUI

  private var vpnServiceConnection: VpnServiceConnection? = null

  private var permissionAlertDialog: AlertDialog? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)

    requireUIProcess()

    setContentView(R.layout.activity_main)

    ui = findViewById(R.id.ui)

    onBackPressedDispatcher.addCallback {
      if (ui.canGoBack) {
        ui.goBack()
      } else {
        isEnabled = false
        onBackPressedDispatcher.onBackPressed()
        isEnabled = true
      }
    }

    osStatus = OsStatus(this)
    preferences = Preferences(this).apply { registerListener(this@MainActivity) }
    vpnServiceConnection =
        VpnServiceConnection(this).also {
          bindService(
              Intent(this, ObscuraVpnService::class.java),
              it,
              BIND_AUTO_CREATE or BIND_IMPORTANT,
          )
        }

    applyColorScheme()
  }

  override fun onStart() {
    super.onStart()

    osStatus.registerCallbacks()
    osStatus.update()
  }

  override fun onNewIntent(intent: Intent) {
    super.onNewIntent(intent)

    intent.data?.let { uri -> this.ui.handleObscuraUri(uri) }
  }

  override fun onResume() {
    super.onResume()

    ui.onResume()
  }

  override fun onPostResume() {
    super.onPostResume()

    val vpnIntent = VpnService.prepare(this)

    if (vpnIntent == null) {
      permissionAlertDialog?.hide()
      permissionAlertDialog = null

      // ask for notification permissions if they're not available
      if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
        if (ContextCompat.checkSelfPermission(
            this,
            Manifest.permission.POST_NOTIFICATIONS,
        ) != PackageManager.PERMISSION_GRANTED) {
          ActivityCompat.requestPermissions(
              this,
              arrayOf(Manifest.permission.POST_NOTIFICATIONS),
              RequestCodes.NOTIFICATIONS.code,
          )
        }
      }

      startForegroundService(Intent(this, ObscuraVpnService::class.java))
    } else if (permissionAlertDialog == null) {
      permissionAlertDialog =
          AlertDialog.Builder(this)
              .apply {
                setCancelable(false)

                setTitle(R.string.vpn_dialog_title_grant_permission)

                if (preferences.permissionGiven) {
                  // user previously gave permission but now it's been revoked
                  setMessage(R.string.vpn_dialog_message_grant_permission_after_revoke)
                } else {
                  setMessage(R.string.vpn_dialog_message_grant_permission)
                }

                setPositiveButton(android.R.string.ok) { dialog, which ->
                  permissionAlertDialog?.hide()
                  permissionAlertDialog = null

                  startActivityForResult(vpnIntent, RequestCodes.VPN_PREPARE.code)
                }
              }
              .show()
    } else {
      permissionAlertDialog?.show()
    }
  }

  override fun onActivityResult(
      requestCode: Int,
      resultCode: Int,
      data: Intent?,
  ) {
    super.onActivityResult(requestCode, resultCode, data)

    if (requestCode == RequestCodes.VPN_PREPARE.code) {
      if (resultCode == RESULT_OK) {
        preferences.permissionGiven = true

        startForegroundService(Intent(this, ObscuraVpnService::class.java))
      }
    }
  }

  override fun onPause() {
    super.onPause()

    ui.onPause()
  }

  override fun onStop() {
    super.onStop()

    osStatus.deregisterCallbacks()
    osStatus.update()
  }

  override fun onDestroy() {
    super.onDestroy()

    preferences.unregisterListener(this)
    vpnServiceConnection?.let { unbindService(it) }
  }

  override fun onConfigurationChanged(newConfig: Configuration) {
    super.onConfigurationChanged(newConfig)

    logDebug("configuration changed: $newConfig")

    this.ui.invalidate()
  }

  override fun onSharedPreferenceChanged(
      sharedPreferences: SharedPreferences?,
      key: String?,
  ) {
    if (key == "color-scheme") {
      applyColorScheme()
    }
  }

  private fun applyColorScheme() {
    AppCompatDelegate.setDefaultNightMode(
        when (preferences.colorScheme) {
          SetColorScheme.ColorScheme.Auto -> AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM
          SetColorScheme.ColorScheme.Dark -> AppCompatDelegate.MODE_NIGHT_YES
          SetColorScheme.ColorScheme.Light -> AppCompatDelegate.MODE_NIGHT_NO
        },
    )
  }
}

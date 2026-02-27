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
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.app.AppCompatDelegate
import androidx.core.content.ContextCompat
import androidx.core.view.WindowCompat
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

    private val vpnPermissionRequestLauncher: ActivityResultLauncher<Intent> = this.registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        logDebug("VPN start activity result: $result")
        if (result.resultCode == RESULT_OK) {
            this.startVpnService()
        }
    }
    private val notificationPermissionRequestLauncher: ActivityResultLauncher<String> = this.registerForActivityResult(ActivityResultContracts.RequestPermission()) { isGranted ->
        // We don't actually care if we're granted permission, since this is
        // just the user's preference between "classic" foreground service
        // notifications vs. the modern Task Manager.
        logDebug("notification permission request activity result: $isGranted")
    }

  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)

    requireUIProcess()

        // Edge-to-edge is the future for Android
        // https://developer.android.com/develop/ui/views/layout/edge-to-edge
        WindowCompat.enableEdgeToEdge(this.window)

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

    fun startVpnService() {
        logDebug("starting VPN service")
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU
            && ContextCompat.checkSelfPermission(
                this,
                Manifest.permission.POST_NOTIFICATIONS,
            ) != PackageManager.PERMISSION_GRANTED
        ) {
            this.notificationPermissionRequestLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
        this.startForegroundService(Intent(this, ObscuraVpnService::class.java))
    }

    // TODO: https://linear.app/soveng/issue/OBS-3192/onpostresume-is-the-wrong-place-to-start-the-vpnservice
    override fun onPostResume() {
        super.onPostResume()

        logDebug("onPostResume")

        // TODO: https://linear.app/soveng/issue/OBS-3193/vpnserviceprepare-isnt-handled-exhaustively
        val vpnIntent = VpnService.prepare(this)
        if (vpnIntent == null) {
            // We already have VPN permission
            this.startVpnService()
        } else {
            // Request VPN permission
            this.vpnPermissionRequestLauncher.launch(vpnIntent)
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

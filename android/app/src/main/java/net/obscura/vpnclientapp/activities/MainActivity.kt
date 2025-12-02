package net.obscura.vpnclientapp.activities

import android.Manifest
import android.content.Intent
import android.content.SharedPreferences
import android.content.pm.PackageManager
import android.net.VpnService
import android.os.Build
import android.os.Bundle
import android.view.ViewGroup
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import androidx.activity.addCallback
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.app.AppCompatDelegate
import androidx.core.app.ActivityCompat
import androidx.core.content.ContextCompat
import net.obscura.vpnclientapp.R
import net.obscura.vpnclientapp.helpers.currentApp
import net.obscura.vpnclientapp.preferences.Preferences
import net.obscura.vpnclientapp.services.ObscuraVpnService
import net.obscura.vpnclientapp.ui.ObscuraWebView
import net.obscura.vpnclientapp.ui.commands.SetColorScheme

class MainActivity : AppCompatActivity(), SharedPreferences.OnSharedPreferenceChangeListener {
  enum class RequestCodes(
      val code: Int,
  ) {
    VPN_PREPARE(111111),
    NOTIFICATIONS(222222),
  }

  private lateinit var preferences: Preferences

  private var permissionAlertDialog: AlertDialog? = null

  override fun onCreate(savedInstanceState: Bundle?) {
    super.onCreate(savedInstanceState)

    preferences = Preferences(this).apply { registerListener(this@MainActivity) }

    applyColorScheme()

    ObscuraWebView(this).also { webView ->
      setContentView(
          webView,
          ViewGroup.LayoutParams(MATCH_PARENT, MATCH_PARENT),
      )

      onBackPressedDispatcher.addCallback {
        if (webView.canGoBack()) {
          webView.goBack()
        } else {
          isEnabled = false
          onBackPressedDispatcher.onBackPressed()
          isEnabled = true
        }
      }
    }
  }

  override fun onResume() {
    super.onResume()

    currentApp().osStatus.update()
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

  override fun onDestroy() {
    super.onDestroy()

    preferences.unregisterListener(this)
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

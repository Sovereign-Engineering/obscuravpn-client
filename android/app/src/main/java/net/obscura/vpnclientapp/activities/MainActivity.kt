package net.obscura.vpnclientapp.activities

import android.content.SharedPreferences
import android.os.Bundle
import android.view.ViewGroup
import android.view.ViewGroup.LayoutParams.MATCH_PARENT
import androidx.activity.addCallback
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.app.AppCompatDelegate
import net.obscura.vpnclientapp.helpers.currentApp
import net.obscura.vpnclientapp.preferences.Preferences
import net.obscura.vpnclientapp.ui.ObscuraWebView
import net.obscura.vpnclientapp.ui.commands.SetColorScheme

class MainActivity : AppCompatActivity(), SharedPreferences.OnSharedPreferenceChangeListener {
  lateinit var preferences: Preferences

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

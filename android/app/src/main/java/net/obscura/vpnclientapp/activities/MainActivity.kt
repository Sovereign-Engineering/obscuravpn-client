package net.obscura.vpnclientapp.activities

import android.content.ComponentName
import android.content.Intent
import android.content.ServiceConnection
import android.content.SharedPreferences
import android.content.res.Configuration
import android.os.Bundle
import android.os.IBinder
import androidx.activity.addCallback
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.app.AppCompatDelegate
import androidx.core.view.WindowCompat
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.BillingFacade
import net.obscura.vpnclientapp.R
import net.obscura.vpnclientapp.helpers.requireUIProcess
import net.obscura.vpnclientapp.preferences.Preferences
import net.obscura.vpnclientapp.services.IObscuraVpnService
import net.obscura.vpnclientapp.services.bindVpnService
import net.obscura.vpnclientapp.services.unbindVpnService
import net.obscura.vpnclientapp.ui.ObscuraUI
import net.obscura.vpnclientapp.ui.OsStatusManager
import net.obscura.vpnclientapp.ui.VpnPermissionRequestManager

private val log = Logger(MainActivity::class)

@AndroidEntryPoint
class MainActivity : AppCompatActivity(), ServiceConnection, SharedPreferences.OnSharedPreferenceChangeListener {
    @Inject lateinit var billingFacade: BillingFacade
    @Inject lateinit var osStatusManager: OsStatusManager
    @Inject lateinit var vpnPermissionRequestManager: VpnPermissionRequestManager

    private lateinit var preferences: Preferences

    private lateinit var ui: ObscuraUI

    private var isFreshLaunch: Boolean = true
    private var isVpnServiceBound: Boolean = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        requireUIProcess()

        this.isFreshLaunch = savedInstanceState == null

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

        preferences = Preferences(this).apply { registerListener(this@MainActivity) }

        applyColorScheme()

        this.isVpnServiceBound = this.bindVpnService(this)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)

        intent.data?.let { uri -> this.ui.handleObscuraUri(uri) }
    }

    override fun onResume() {
        super.onResume()

        ui.onResume()
    }

    override fun onPause() {
        super.onPause()

        ui.onPause()
    }

    override fun onDestroy() {
        super.onDestroy()
        this.preferences.unregisterListener(this)
        if (this.isVpnServiceBound) {
            this.unbindVpnService(this)
        }
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)

        log.debug("configuration changed: $newConfig")

        this.ui.invalidate()
    }

    override fun onServiceConnected(name: ComponentName?, service: IBinder?) {
        log.debug("onServiceConnected $name $service")
        this.ui.onCreate(
            this.isFreshLaunch,
            IObscuraVpnService.Stub.asInterface(service),
            this,
            this.osStatusManager,
        )
        this.isFreshLaunch = false
    }

    override fun onServiceDisconnected(name: ComponentName?) {
        log.debug("onServiceDisconnected $name")
        this.ui.onDestroy()
    }

    override fun onSharedPreferenceChanged(sharedPreferences: SharedPreferences?, key: String?) {
        if (key == "color-scheme") {
            applyColorScheme()
        }
    }

    private fun applyColorScheme() {
        AppCompatDelegate.setDefaultNightMode(
            when (this.preferences.colorScheme) {
                Preferences.ColorScheme.Auto -> AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM
                Preferences.ColorScheme.Dark -> AppCompatDelegate.MODE_NIGHT_YES
                Preferences.ColorScheme.Light -> AppCompatDelegate.MODE_NIGHT_NO
            }
        )
    }
}

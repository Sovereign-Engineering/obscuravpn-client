package net.obscura.vpnclientapp.activities

import android.content.ComponentName
import android.content.Intent
import android.content.ServiceConnection
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
import net.obscura.vpnclientapp.R
import net.obscura.vpnclientapp.services.IObscuraVpnService
import net.obscura.vpnclientapp.services.bindVpnService
import net.obscura.vpnclientapp.services.unbindVpnService
import net.obscura.vpnclientapp.ui.BillingFacade
import net.obscura.vpnclientapp.ui.ObscuraUI
import net.obscura.vpnclientapp.ui.OsStatus
import net.obscura.vpnclientapp.ui.OsStatusManager
import net.obscura.vpnclientapp.ui.PreferencesManager
import net.obscura.vpnclientapp.ui.VpnPermissionRequestManager

private val log = Logger(MainActivity::class)

@AndroidEntryPoint
class MainActivity : AppCompatActivity(), ServiceConnection {
    @Inject lateinit var billingFacade: BillingFacade
    @Inject lateinit var osStatusManager: OsStatusManager
    lateinit var preferencesManager: PreferencesManager
    @Inject lateinit var vpnPermissionRequestManager: VpnPermissionRequestManager

    private lateinit var ui: ObscuraUI

    private var isFreshLaunch: Boolean = true
    private var isVpnServiceBound: Boolean = false

    private fun handleIntent(intent: Intent?) = intent?.data?.let { uri -> this.ui.handleObscuraUri(this, uri) }

    override fun onCreate(savedInstanceState: Bundle?) {
        log.trace("onCreate")
        super.onCreate(savedInstanceState)

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

        this.isVpnServiceBound = this.bindVpnService(this)
        this.preferencesManager =
            PreferencesManager(this) { preferences ->
                log.trace("onPreferencesChanged $preferences")
                AppCompatDelegate.setDefaultNightMode(
                    when (preferences.colorScheme) {
                        OsStatus.ColorScheme.Auto -> AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM
                        OsStatus.ColorScheme.Dark -> AppCompatDelegate.MODE_NIGHT_YES
                        OsStatus.ColorScheme.Light -> AppCompatDelegate.MODE_NIGHT_NO
                    }
                )
                this@MainActivity.osStatusManager.update { this.colorScheme = preferences.colorScheme }
            }
    }

    override fun onNewIntent(intent: Intent) {
        log.trace("onNewIntent")
        super.onNewIntent(intent)
        this.handleIntent(intent)
    }

    override fun onResume() {
        log.trace("onResume")
        super.onResume()
        this.ui.onResume()
    }

    override fun onPause() {
        log.trace("onPause")
        super.onPause()
        this.ui.onPause()
    }

    override fun onDestroy() {
        log.trace("onDestroy")
        super.onDestroy()
        if (this.isVpnServiceBound) {
            this.unbindVpnService(this)
        }
        if (::ui.isInitialized) {
            this.ui.onDestroy()
        }
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        log.trace("onConfigurationChanged $newConfig")
        super.onConfigurationChanged(newConfig)
        this.ui.invalidate()
    }

    override fun onServiceConnected(name: ComponentName?, service: IBinder?) {
        log.trace("onServiceConnected $name $service")
        this.ui.onCreate(
            this.isFreshLaunch,
            IObscuraVpnService.Stub.asInterface(service),
            this,
            this.osStatusManager,
        )
        // `getIntent` is preserved across activity recreation.
        if (this.isFreshLaunch) this.handleIntent(this.intent)
        this.isFreshLaunch = false
    }

    override fun onServiceDisconnected(name: ComponentName?) {
        log.trace("onServiceDisconnected $name")
        this.ui.onDestroy()
    }
}

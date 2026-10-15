package net.obscura.vpnclientapp.activities

import android.content.ComponentName
import android.content.Intent
import android.content.ServiceConnection
import android.content.pm.PackageManager
import android.content.res.Configuration
import android.os.Bundle
import android.os.IBinder
import androidx.activity.addCallback
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.app.AppCompatDelegate
import androidx.core.content.ContextCompat
import androidx.core.view.WindowCompat
import dagger.hilt.android.AndroidEntryPoint
import javax.inject.Inject
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.BuildConfig
import net.obscura.vpnclientapp.R
import net.obscura.vpnclientapp.services.IObscuraVpnService
import net.obscura.vpnclientapp.services.bindVpnService
import net.obscura.vpnclientapp.services.unbindVpnService
import net.obscura.vpnclientapp.ui.BillingFacade
import net.obscura.vpnclientapp.ui.DeepLinkOrigin
import net.obscura.vpnclientapp.ui.ObscuraUI
import net.obscura.vpnclientapp.ui.OsStatus
import net.obscura.vpnclientapp.ui.OsStatusManager
import net.obscura.vpnclientapp.ui.PreferencesManager
import net.obscura.vpnclientapp.ui.VpnPermissionRequestManager

private val log = Logger(MainActivity::class)

/**
 * Signature-level custom permission held only by apps signed with our signing key (i.e. us).
 * Senders that were granted it are trusted to drive *privileged* deep links, such as
 * `requestVpnStart`.
 */
const val PERMISSION_TRUSTED_DEEP_LINK = "${BuildConfig.APPLICATION_ID}.permission.TRUSTED_DEEP_LINK"

@AndroidEntryPoint
class MainActivity : AppCompatActivity(), ServiceConnection {
    @Inject lateinit var billingFacade: BillingFacade
    @Inject lateinit var osStatusManager: OsStatusManager
    lateinit var preferencesManager: PreferencesManager
    @Inject lateinit var vpnPermissionRequestManager: VpnPermissionRequestManager

    private lateinit var ui: ObscuraUI

    private var isFreshLaunch: Boolean = true
    private var isVpnServiceBound: Boolean = false

    private fun handleIntent(intent: Intent?) {
        if (intent == null || intent.data == null) return

        // The platform only grants a signature-level permission if the sender's signing
        // certificate matches ours (kernel/PMS-enforced); third-party apps and browsers can
        // neither hold nor cause this grant, and cannot forge it via intent extras. Our own
        // notification PendingIntents satisfy this because the manifest declares and uses this
        // permission itself.
        val isInternalOrigin =
            ContextCompat.checkSelfPermission(this, PERMISSION_TRUSTED_DEEP_LINK) ==
                PackageManager.PERMISSION_GRANTED

        val deepLinkOrigin =
            if (isInternalOrigin) {
                DeepLinkOrigin.Internal
            } else {
                DeepLinkOrigin.External
            }

        log.debug("deep-link origin: $deepLinkOrigin")
        this.ui.handleObscuraUri(this, intent.data!!, deepLinkOrigin)
    }

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
        // Mirror the platform contract: referrer/state only reflect a new intent after
        // `setIntent`; we derive provenance from the incoming intent itself.
        this.setIntent(intent)
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

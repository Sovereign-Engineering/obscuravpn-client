package net.obscura.vpnclientapp.ui

import android.content.Context
import android.net.Uri
import android.util.AttributeSet
import android.widget.FrameLayout
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.lifecycle.lifecycleScope
import com.google.android.material.bottomnavigation.BottomNavigationView
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.launch
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.R
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.ErrorCodeException
import net.obscura.vpnclientapp.client.ManagerCmdOk
import net.obscura.vpnclientapp.services.IObscuraVpnService
import net.obscura.vpnclientapp.ui.bridge.WebCmdArgs

private val log = Logger(ObscuraUI::class)

private fun viewFromId(id: Int) =
    when (id) {
        R.id.nav_connection -> OsStatus.NavigationView.Connection
        R.id.nav_location -> OsStatus.NavigationView.Location
        R.id.nav_account -> OsStatus.NavigationView.Account
        R.id.nav_settings -> OsStatus.NavigationView.Settings
        R.id.nav_about -> OsStatus.NavigationView.About
        else -> {
            log.error("unrecognized view id: $id")
            null
        }
    }

private fun idFromView(view: OsStatus.NavigationView) =
    when (view) {
        OsStatus.NavigationView.Connection -> R.id.nav_connection
        OsStatus.NavigationView.Location -> R.id.nav_location
        OsStatus.NavigationView.Account -> R.id.nav_account
        OsStatus.NavigationView.Settings -> R.id.nav_settings
        OsStatus.NavigationView.About -> R.id.nav_about
        else -> {
            log.error("view has no id: $view")
            null
        }
    }

class ObscuraUI @JvmOverloads constructor(context: Context, attrs: AttributeSet? = null) : FrameLayout(context, attrs) {
    private var purchaseTokenUploader: Job? = null
    private lateinit var vpnStatusObserver: VpnStatusObserver

    val canGoBack
        get() = bottomNavigation.selectedItemId != R.id.nav_connection

    private lateinit var webViewContainer: FrameLayout
    private lateinit var bottomNavigation: BottomNavigationView
    private var loggedIn: Boolean = false

    private var webView: ObscuraWebView? = null

    private fun setLoggedIn(loggedIn: Boolean) {
        this.bottomNavigation.visibility = if (loggedIn) VISIBLE else GONE
        this.loggedIn = loggedIn
    }

    override fun onFinishInflate() {
        super.onFinishInflate()

        this.webViewContainer = this.findViewById(R.id.web_view_container)
        this.bottomNavigation = this.findViewById(R.id.nav_view)
        this.bottomNavigation.visibility = GONE

        // TODO: Synchronize padding with IME animation
        // https://linear.app/soveng/issue/OBS-3233/android-ime-animation-jank
        ViewCompat.setOnApplyWindowInsetsListener(this.webViewContainer) { view, windowInsets ->
            val insetsMask = WindowInsetsCompat.Type.displayCutout().or(WindowInsetsCompat.Type.systemBars())
            val insets = windowInsets.getInsets(insetsMask)
            val imeMask = WindowInsetsCompat.Type.ime()
            val bottom =
                if (windowInsets.isVisible(imeMask)) {
                    // Injecting this would cause a slight resize of the web UI
                    Pair(windowInsets.getInsets(imeMask).bottom, 0)
                } else if (!this.loggedIn) {
                    Pair(0, insets.bottom)
                } else {
                    Pair(0, 0)
                }
            view.setPadding(0, 0, 0, bottom.first)
            this.webView?.injectInsets(Insets.of(insets.left, insets.top, insets.right, bottom.second))
            WindowInsetsCompat.CONSUMED
        }
        ViewCompat.setOnApplyWindowInsetsListener(this.bottomNavigation) { view, windowInsets ->
            // Hide bottom nav when IME is visible
            // https://github.com/software-mansion/react-native-screens/issues/3647
            val showBottomNav = this.loggedIn && !windowInsets.isVisible(WindowInsetsCompat.Type.ime())
            view.visibility = if (showBottomNav) VISIBLE else GONE
            val systemBars = windowInsets.getInsets(WindowInsetsCompat.Type.systemBars())
            view.setPadding(systemBars.left, 0, systemBars.right, systemBars.bottom)
            WindowInsetsCompat.CONSUMED
        }
    }

    fun onCreate(
        isFreshLaunch: Boolean,
        binder: IObscuraVpnService,
        mainActivity: MainActivity,
        osStatusManager: OsStatusManager,
    ) {
        onDestroy()

        this.purchaseTokenUploader =
            mainActivity.lifecycleScope.launch(Dispatchers.Default) {
                val purchaseTokens = mainActivity.billingFacade.fetchPurchaseTokens()
                if (purchaseTokens != null) {
                    for (purchaseToken in purchaseTokens) {
                        try {
                            uploadPurchaseToken(binder, purchaseToken, null)
                        } catch (_: ErrorCodeException) {
                            // This is already logged internally
                        }
                    }
                }
            }

        this.vpnStatusObserver =
            VpnStatusObserver(
                binder,
                object : VpnStatusObserver.Callback {
                    private var isAutoConnectEligible = isFreshLaunch

                    override suspend fun onStatusChanged(status: ManagerCmdOk.GetStatus) {
                        osStatusManager.update {
                            this.vpnStatus =
                                when (status.vpnStatus) {
                                    is ManagerCmdOk.GetStatus.VpnStatus.Connected -> OsStatus.OsVpnStatus.Connected
                                    is ManagerCmdOk.GetStatus.VpnStatus.Connecting -> OsStatus.OsVpnStatus.Connecting
                                    is ManagerCmdOk.GetStatus.VpnStatus.Disconnected ->
                                        OsStatus.OsVpnStatus.Disconnected
                                }
                        }
                        this@ObscuraUI.setLoggedIn(status.accountId != null && !status.inNewAccountFlow)
                        val shouldAutoConnect =
                            this.isAutoConnectEligible &&
                                status.autoConnect &&
                                status.vpnStatus is ManagerCmdOk.GetStatus.VpnStatus.Disconnected
                        this.isAutoConnectEligible = false
                        if (shouldAutoConnect) {
                            mainActivity.vpnPermissionRequestManager
                                .requestVpnStart()
                                .mapCatching { binder.startTunnel(null) }
                                .onSuccess { log.info("auto-connected VPN") }
                                .onFailure { log.error("failed to auto-connect VPN: ${it.message}", tr = it) }
                        }
                    }
                },
            )
        mainActivity.lifecycle.addObserver(this.vpnStatusObserver)

        this.bottomNavigation.setOnItemSelectedListener {
            val view = viewFromId(it.itemId)
            val didNavigate = view?.serialName()?.let { path -> this@ObscuraUI.webView?.navigate(path) } != null
            if (didNavigate) osStatusManager.update { this.navigationView = view }
            didNavigate
        }
        osStatusManager.update { this.navigationView = viewFromId(this@ObscuraUI.bottomNavigation.selectedItemId) }
        this.webView =
            ObscuraWebView(WebCmdArgs(context, binder, mainActivity, osStatusManager, this)).also {
                this.webViewContainer.addView(
                    it,
                    LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT),
                )
            }
    }

    fun onResume() {
        webView?.onResume()
    }

    fun onPause() {
        webView?.onPause()
    }

    fun onDestroy() {
        this.purchaseTokenUploader?.cancel(CancellationException("UI destroyed"))

        bottomNavigation.visibility = GONE
        webViewContainer.removeAllViews()

        this.webView?.bridge?.cancel()
        this.webView?.destroy()
        this.webView = null
    }

    override fun invalidate() {
        super.invalidate()

        this.webView?.invalidate()
    }

    fun goBack() {
        bottomNavigation.selectedItemId = R.id.nav_connection
    }

    fun setNavigationView(view: OsStatus.NavigationView) =
        idFromView(view)?.let { this.bottomNavigation.selectedItemId = it }

    fun handleObscuraUri(uri: Uri) {
        log.debug("handling deep link: $uri")
        val id =
            when (uri.path) {
                "/account" -> R.id.nav_account
                "/location" -> R.id.nav_location
                else -> {
                    log.error("unrecognized path for deep link: $uri")
                    return
                }
            }
        this.bottomNavigation.selectedItemId = id
    }
}

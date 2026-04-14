package net.obscura.vpnclientapp.ui

import android.content.Context
import android.net.Uri
import android.util.AttributeSet
import android.widget.FrameLayout
import androidx.core.graphics.Insets
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.postDelayed
import com.google.android.material.bottomnavigation.BottomNavigationView
import com.google.android.material.navigation.NavigationBarView
import java.lang.ref.WeakReference
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.R
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.ManagerCmd
import net.obscura.vpnclientapp.client.ManagerCmdOk
import net.obscura.vpnclientapp.client.jsonConfig
import net.obscura.vpnclientapp.services.IObscuraVpnService
import net.obscura.vpnclientapp.ui.bridge.WebCmdBridge

private val log = Logger(ObscuraUI::class)

class ObscuraUI @JvmOverloads constructor(context: Context, attrs: AttributeSet? = null) : FrameLayout(context, attrs) {
    private class StatusObserver(
        val binder: WeakReference<IObscuraVpnService>,
        val onStatusChanged: (ManagerCmdOk.GetStatus) -> Unit,
    ) {
        private var enabled = true
        private var knownVersion: String? = null

        fun observe() {
            synchronized(this) {
                binder.get()?.let { binder ->
                    WebCmdBridge.Receiver.register {
                            binder.jsonFfi(
                                it,
                                jsonConfig.encodeToString(ManagerCmd.GetStatus(knownVersion)),
                            )
                        }
                        .handle { data, _ -> data?.let { onStatusUpdated(jsonConfig.decodeFromString(it)) } }
                }
            }
        }

        fun disable() {
            synchronized(this) { enabled = false }
        }

        private fun onStatusUpdated(status: ManagerCmdOk.GetStatus) {
            synchronized(this) {
                knownVersion = status.version

                if (enabled) {
                    onStatusChanged(status)
                    observe()
                }
            }
        }
    }

    val canGoBack
        get() = (webView?.canGoBack() ?: false) || (bottomNavigation.selectedItemId != R.id.nav_connection)

    private var statusObserver: StatusObserver? = null

    private lateinit var webViewContainer: FrameLayout
    private lateinit var bottomNavigation: BottomNavigationView
    private var loggedIn: Boolean = false

    private var webView: ObscuraWebView? = null

    private val itemReselectedListener = NavigationBarView.OnItemReselectedListener { navigateToTab(it.itemId) }

    private val itemSelectedListener =
        NavigationBarView.OnItemSelectedListener {
            navigateToTab(it.itemId)

            true
        }

    private fun setLoggedIn(loggedIn: Boolean) {
        this.bottomNavigation.visibility = if (loggedIn) VISIBLE else GONE
        this.loggedIn = loggedIn
    }

    override fun onFinishInflate() {
        super.onFinishInflate()

        this.webViewContainer = this.findViewById(R.id.web_view_container)
        this.bottomNavigation = this.findViewById(R.id.nav_view)
        this.bottomNavigation.visibility = GONE
        this.bottomNavigation.setOnItemReselectedListener(itemReselectedListener)
        this.bottomNavigation.setOnItemSelectedListener(itemSelectedListener)

        // TODO: Synchronize padding with IME animation
        // https://linear.app/soveng/issue/OBS-3233/android-ime-animation-jank
        // TODO: Edge-to-edge `WebView`
        // https://linear.app/soveng/issue/OBS-3237/android-edge-to-edge-webview
        ViewCompat.setOnApplyWindowInsetsListener(this.webViewContainer) { view, windowInsets ->
            val insetsMask =
                WindowInsetsCompat.Type.displayCutout()
                    .or(WindowInsetsCompat.Type.navigationBars())
                    .or(WindowInsetsCompat.Type.statusBars())
            val insets = windowInsets.getInsets(insetsMask)
            val imeMask = WindowInsetsCompat.Type.ime()
            val bottom =
                if (windowInsets.isVisible(imeMask)) {
                    windowInsets.getInsets(imeMask).bottom
                } else if (!this.loggedIn) {
                    insets.bottom
                } else {
                    0
                }
            // Only use non-zero insets when there's overlap
            // https://developer.android.com/develop/ui/views/layout/webapps/understand-window-insets#bounds-overlap
            view.setPadding(insets.left, insets.top, insets.right, bottom)
            // Child `WebView` should ignore any insets we applied here
            // https://developer.android.com/develop/ui/views/layout/webapps/understand-window-insets#inset-handling
            WindowInsetsCompat.Builder(windowInsets).setInsets(insetsMask.or(imeMask), Insets.NONE).build()
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

    fun onCreate(binder: IObscuraVpnService, mainActivity: MainActivity, osStatusManager: OsStatusManager) {
        onDestroy()

        webView =
            ObscuraWebView(context, binder, mainActivity, osStatusManager).apply {
                webViewContainer.addView(
                    this,
                    LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT),
                )

                onPageLoadedCallback = {
                    if (bottomNavigation.selectedItemId != R.id.nav_connection) {
                        // TODO: make sure UI picks this up correctly

                        var delay = 0L
                        while (delay < 100L) {
                            postDelayed(delay) { navigateToTab(bottomNavigation.selectedItemId) }
                            delay += 10
                        }
                    }
                }

                statusObserver?.disable()
                statusObserver =
                    StatusObserver(WeakReference(binder)) {
                            osStatusManager.update {
                                this.vpnStatus =
                                    when (it.vpnStatus) {
                                        is ManagerCmdOk.GetStatus.VpnStatus.Connected -> OsStatus.OsVpnStatus.Connected
                                        is ManagerCmdOk.GetStatus.VpnStatus.Connecting ->
                                            OsStatus.OsVpnStatus.Connecting
                                        is ManagerCmdOk.GetStatus.VpnStatus.Disconnected ->
                                            OsStatus.OsVpnStatus.Disconnected
                                    }
                            }
                            this@ObscuraUI.setLoggedIn(it.accountId != null && !it.inNewAccountFlow)
                        }
                        .apply { observe() }
            }
    }

    fun onResume() {
        webView?.onResume()
    }

    fun onPause() {
        webView?.onPause()
    }

    fun onDestroy() {
        statusObserver?.disable()
        statusObserver = null

        bottomNavigation.visibility = GONE
        webViewContainer.removeAllViews()

        webView?.destroy()
        webView = null
    }

    override fun invalidate() {
        super.invalidate()

        this.webView?.invalidate()
    }

    fun goBack() {
        if (webView?.canGoBack() ?: false) {
            webView?.goBack()
        } else if (bottomNavigation.selectedItemId != R.id.nav_connection) {
            bottomNavigation.selectedItemId = R.id.nav_connection
        }
    }

    private fun navigateToTab(id: Int) {
        val path =
            when (id) {
                R.id.nav_connection -> ""
                R.id.nav_location -> "location"
                R.id.nav_account -> "account"
                R.id.nav_settings -> "settings"
                R.id.nav_about -> "about"
                else -> {
                    log.error("unrecognized view id: $id")
                    return
                }
            }
        this.webView?.navigate(path)
    }

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

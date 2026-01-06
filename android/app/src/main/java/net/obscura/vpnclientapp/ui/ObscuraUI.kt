package net.obscura.vpnclientapp.ui

import android.content.Context
import android.util.AttributeSet
import android.widget.FrameLayout
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.postDelayed
import com.google.android.material.bottomnavigation.BottomNavigationView
import com.google.android.material.navigation.NavigationBarView
import java.lang.ref.WeakReference
import kotlinx.coroutines.delay
import kotlinx.serialization.json.Json
import net.obscura.vpnclientapp.R
import net.obscura.vpnclientapp.client.commands.GetStatus
import net.obscura.vpnclientapp.services.IObscuraVpnService

class ObscuraUI
@JvmOverloads
constructor(
    context: Context,
    attrs: AttributeSet? = null,
) : FrameLayout(context, attrs) {
  private class StatusObserver(
      val binder: WeakReference<IObscuraVpnService>,
      val onStatusChanged: (GetStatus.Response) -> Unit,
  ) {
    private val json = Json { ignoreUnknownKeys = true }

    private var enabled = true
    private var knownVersion: String? = null

    fun observe() {
      synchronized(this) {
        binder.get()?.let { binder ->
          CommandBridge.Receiver.register {
                binder.jsonFfi(
                    it,
                    json.encodeToString(GetStatus(GetStatus.Request(knownVersion = knownVersion))),
                )
              }
              .handle { data, exception ->
                data?.let { onStatusUpdated(json.decodeFromString(it)) }
              }
        }
      }
    }

    fun disable() {
      synchronized(this) { enabled = false }
    }

    private fun onStatusUpdated(status: GetStatus.Response) {
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
    get() =
        (webView?.canGoBack() ?: false) || (bottomNavigation.selectedItemId != R.id.nav_connection)

  private var statusObserver: StatusObserver? = null

  private lateinit var webViewContainer: FrameLayout
  private lateinit var bottomNavigation: BottomNavigationView

  private var webView: ObscuraWebView? = null

  private val itemReselectedListener =
      NavigationBarView.OnItemReselectedListener { navigateToTab(it.itemId) }

  private val itemSelectedListener =
      NavigationBarView.OnItemSelectedListener {
        navigateToTab(it.itemId)

        true
      }

  override fun onFinishInflate() {
    super.onFinishInflate()

    webViewContainer = findViewById(R.id.web_view_container)
    bottomNavigation = findViewById(R.id.nav_view)

    bottomNavigation.visibility = GONE
    bottomNavigation.setOnItemReselectedListener(itemReselectedListener)
    bottomNavigation.setOnItemSelectedListener(itemSelectedListener)

    ViewCompat.setOnApplyWindowInsetsListener(this) { _, insets ->
      insets.getInsets(WindowInsetsCompat.Type.systemBars()).let { systemBars ->
        webViewContainer.setPadding(
            systemBars.left,
            systemBars.top,
            systemBars.right,
            webViewContainer.paddingBottom,
        )

        bottomNavigation.setPadding(
            systemBars.left,
            bottomNavigation.paddingTop,
            systemBars.right,
            systemBars.bottom,
        )
      }

      insets
    }
  }

  fun onCreate(
      binder: IObscuraVpnService,
      osStatus: OsStatus,
  ) {
    onDestroy()

    webView =
        ObscuraWebView(context, binder, osStatus).apply {
          webViewContainer.addView(
              this,
              LayoutParams(
                  LayoutParams.MATCH_PARENT,
                  LayoutParams.MATCH_PARENT,
              ),
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
              StatusObserver(
                      WeakReference(binder),
                      {
                        bottomNavigation.visibility =
                            if (it.accountId == null || it.inNewAccountFlow) {
                              GONE
                            } else {
                              VISIBLE
                            }
                      },
                  )
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

  fun goBack() {
    if (webView?.canGoBack() ?: false) {
      webView?.goBack()
    } else if (bottomNavigation.selectedItemId != R.id.nav_connection) {
      bottomNavigation.selectedItemId = R.id.nav_connection
    }
  }

  private fun navigateToTab(id: Int) {
    webView?.navigate(
        when (id) {
          R.id.nav_connection -> ""
          R.id.nav_location -> "location"
          R.id.nav_account -> "account"
          R.id.nav_settings -> "settings"
          R.id.nav_about -> "about"
          else -> throw RuntimeException("Unknown id")
        },
    )
  }
}

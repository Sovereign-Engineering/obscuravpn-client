package net.obscura.vpnclientapp.ui

import android.content.Context
import android.content.Intent
import android.util.AttributeSet
import android.webkit.WebMessage
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.core.net.toUri
import androidx.webkit.WebViewAssetLoader
import net.obscura.vpnclientapp.helpers.alwaysHTTPS
import net.obscura.vpnclientapp.helpers.whenTrue
import net.obscura.vpnclientapp.services.IObscuraVpnService

class ObscuraWebView
@JvmOverloads
constructor(
    context: Context,
    binder: IObscuraVpnService,
    osStatus: OsStatus,
    attrs: AttributeSet? = null,
) : WebView(context, attrs) {
  companion object {
    val ORIGIN = "https://appassets.androidplatform.net".toUri()

    val HOME = "$ORIGIN/assets/index.html"
  }

  val commandBridge =
      CommandBridge(context, binder, osStatus) { data ->
        post { postWebMessage(WebMessage("android/$data"), ORIGIN) }
      }

  init {
    settings.javaScriptEnabled = true

    addJavascriptInterface(commandBridge, "obscuraAndroidCommandBridge")

    WebViewAssetLoader.Builder()
        .addPathHandler("/assets/", WebViewAssetLoader.AssetsPathHandler(context))
        .addPathHandler("/res/", WebViewAssetLoader.ResourcesPathHandler(context))
        .build()
        .also { assetLoader ->
          webViewClient =
              object : WebViewClient() {
                override fun shouldOverrideUrlLoading(
                    view: WebView,
                    request: WebResourceRequest,
                ) =
                    (request.url.host != ORIGIN.host).whenTrue {
                      if (request.isForMainFrame) {
                        context.startActivity(
                            Intent(
                                Intent.ACTION_VIEW,
                                request.url.alwaysHTTPS(),
                            ),
                        )
                      }
                    }

                override fun shouldInterceptRequest(
                    view: WebView?,
                    request: WebResourceRequest,
                ) = assetLoader.shouldInterceptRequest(request.url)
              }
        }

    loadUrl(HOME)
  }
}

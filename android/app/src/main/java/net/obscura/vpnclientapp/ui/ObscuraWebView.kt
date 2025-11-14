package net.obscura.vpnclientapp.ui

import android.content.Context
import android.util.AttributeSet
import android.webkit.WebMessage
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.core.net.toUri
import androidx.webkit.WebViewAssetLoader

class ObscuraWebView
@JvmOverloads
constructor(
    context: Context,
    attrs: AttributeSet? = null,
) : WebView(context, attrs) {
  companion object {
    val ORIGIN = "https://appassets.androidplatform.net".toUri()

    val HOME = "$ORIGIN/assets/index.html"
  }

  val commandBridge =
      CommandBridge(context) { data ->
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
                override fun shouldInterceptRequest(
                    view: WebView?,
                    request: WebResourceRequest?,
                ): WebResourceResponse? = assetLoader.shouldInterceptRequest(request!!.url)
              }
        }

    loadUrl(HOME)
  }
}

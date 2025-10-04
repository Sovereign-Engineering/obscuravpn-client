package net.obscura.vpnclientapp.ui

import android.content.Context
import android.util.AttributeSet
import android.util.Log
import android.webkit.WebResourceRequest
import android.webkit.WebResourceResponse
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.webkit.WebViewAssetLoader

class ObscuraWebView @JvmOverloads constructor(
    context: Context, attrs: AttributeSet? = null
) : WebView(context, attrs) {
    companion object {
        const val HOME = "https://appassets.androidplatform.net/assets/index.html"
    }

    val commandBridge = CommandBridge({ js, callback ->
        post {
            evaluateJavascript(js, callback)
        }
    })

    init {
        settings.javaScriptEnabled = true

        addJavascriptInterface(commandBridge, "obscuraAndroidCommandBridge")

        WebViewAssetLoader.Builder()
            .addPathHandler("/assets/", WebViewAssetLoader.AssetsPathHandler(context))
            .addPathHandler("/res/", WebViewAssetLoader.ResourcesPathHandler(context))
            .build()
            .also { assetLoader ->
                webViewClient = object : WebViewClient() {
                    override fun shouldInterceptRequest(
                        view: WebView?,
                        request: WebResourceRequest?
                    ): WebResourceResponse? {
                        return assetLoader.shouldInterceptRequest(request!!.url)
                    }
                }
            }

        loadUrl(HOME)
    }
}

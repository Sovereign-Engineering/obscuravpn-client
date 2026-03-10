package net.obscura.vpnclientapp.ui

import android.annotation.SuppressLint
import android.content.Context
import android.content.Intent
import android.util.AttributeSet
import android.webkit.WebMessage
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.core.net.toUri
import androidx.webkit.WebViewAssetLoader
import net.obscura.vpnclientapp.services.IObscuraVpnService

@SuppressLint("SetJavaScriptEnabled", "ViewConstructor")
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

    var onPageLoadedCallback: ((String) -> Unit)? = null

    init {
        settings.domStorageEnabled = true
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
                        ): Boolean {
                            val shouldOverride = request.url.host != ORIGIN.host
                            if (shouldOverride && request.isForMainFrame) {
                                context.startActivity(
                                    Intent(
                                        Intent.ACTION_VIEW,
                                        if (request.url.scheme == "http") {
                                            request.url.buildUpon().scheme("https").build()
                                        } else {
                                            request.url
                                        },
                                    ),
                                )
                            }
                            return shouldOverride
                        }

                        override fun shouldInterceptRequest(
                            view: WebView?,
                            request: WebResourceRequest,
                        ) = assetLoader.shouldInterceptRequest(request.url)

                        override fun onPageFinished(
                            view: WebView?,
                            url: String,
                        ) {
                            super.onPageFinished(view, url)

                            onPageLoadedCallback?.invoke(url)
                        }
                    }
            }

        loadUrl(HOME)
    }

    fun navigate(path: String) {
        postWebMessage(WebMessage("android-navigate/$path"), ORIGIN)
    }
}

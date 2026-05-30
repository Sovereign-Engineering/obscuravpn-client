package net.obscura.vpnclientapp.ui

import android.annotation.SuppressLint
import android.content.Intent
import android.util.AttributeSet
import android.webkit.WebMessage
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.core.graphics.Insets
import androidx.core.net.toUri
import androidx.core.util.TypedValueCompat.pxToDp
import androidx.webkit.WebViewAssetLoader
import net.obscura.vpnclientapp.ui.bridge.WebCmdArgs
import net.obscura.vpnclientapp.ui.bridge.WebCmdBridge

@SuppressLint("SetJavaScriptEnabled", "ViewConstructor")
class ObscuraWebView
@JvmOverloads
constructor(
    args: WebCmdArgs,
    attrs: AttributeSet? = null,
) : WebView(args.context, attrs) {
    companion object {
        val ORIGIN = "https://appassets.androidplatform.net".toUri()

        val HOME = "$ORIGIN/assets/web/index.html"
    }

    val bridge = WebCmdBridge(args) { data -> post { postWebMessage(WebMessage("android/$data"), ORIGIN) } }

    init {
        this.settings.domStorageEnabled = true
        this.settings.javaScriptEnabled = true
        this.addJavascriptInterface(bridge, "obscuraAndroidCommandBridge")
        this.setRendererPriorityPolicy(RENDERER_PRIORITY_BOUND, true)

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
                                    )
                                )
                            }
                            return shouldOverride
                        }

                        override fun shouldInterceptRequest(
                            view: WebView?,
                            request: WebResourceRequest,
                        ) = assetLoader.shouldInterceptRequest(request.url)

                        override fun onPageFinished(view: WebView?, url: String) {
                            super.onPageFinished(view, url)
                            view?.requestApplyInsets()
                        }
                    }
            }

        loadUrl(HOME)
    }

    // WebView doesn't support edge-to-edge correctly:
    // https://issues.chromium.org/issues/396827865
    // This is the official workaround:
    // https://medium.com/androiddevelopers/make-webviews-edge-to-edge-a6ef319adfac
    fun injectInsets(insets: Insets) {
        val displayMetrics = this.context.resources.displayMetrics
        val top = pxToDp(insets.top.toFloat(), displayMetrics)
        val right = pxToDp(insets.right.toFloat(), displayMetrics)
        val bottom = pxToDp(insets.bottom.toFloat(), displayMetrics)
        val left = pxToDp(insets.left.toFloat(), displayMetrics)
        val safeAreaJs =
            """
            document.documentElement.style.setProperty('--safe-area-inset-top', '${top}px');
            document.documentElement.style.setProperty('--safe-area-inset-right', '${right}px');
            document.documentElement.style.setProperty('--safe-area-inset-bottom', '${bottom}px');
            document.documentElement.style.setProperty('--safe-area-inset-left', '${left}px');
            """
        this.evaluateJavascript(safeAreaJs, null)
    }

    fun navigate(path: String) {
        postWebMessage(WebMessage("android-navigate/$path"), ORIGIN)
    }
}

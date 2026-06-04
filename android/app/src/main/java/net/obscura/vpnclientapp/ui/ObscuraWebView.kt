package net.obscura.vpnclientapp.ui

import android.annotation.SuppressLint
import android.content.Intent
import android.util.AttributeSet
import android.webkit.ConsoleMessage
import android.webkit.WebChromeClient
import android.webkit.WebMessage
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.core.graphics.Insets
import androidx.core.net.toUri
import androidx.core.util.TypedValueCompat.pxToDp
import androidx.webkit.WebViewAssetLoader
import net.obscura.lib.util.LogLevel
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.ui.bridge.WebCmdArgs
import net.obscura.vpnclientapp.ui.bridge.WebCmdBridge

private val log = Logger(ObscuraWebView::class)

@SuppressLint("SetJavaScriptEnabled", "ViewConstructor")
class ObscuraWebView
@JvmOverloads
constructor(
    private val args: WebCmdArgs,
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
        this.webChromeClient =
            object : WebChromeClient() {
                override fun onConsoleMessage(consoleMessage: ConsoleMessage): Boolean {
                    Logger("WebViewConsole")
                        .event(
                            when (consoleMessage.messageLevel()) {
                                ConsoleMessage.MessageLevel.DEBUG -> LogLevel.DEBUG
                                ConsoleMessage.MessageLevel.LOG,
                                ConsoleMessage.MessageLevel.TIP -> LogLevel.INFO
                                ConsoleMessage.MessageLevel.WARNING -> LogLevel.WARN
                                ConsoleMessage.MessageLevel.ERROR -> LogLevel.ERROR
                            },
                            consoleMessage.message(),
                        )
                    return true
                }
            }
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
                            if (request.isForMainFrame) {
                                if (request.url.host == ORIGIN.host) {
                                    log.warn("ignoring URL change: ${request.url.path}")
                                } else {
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
                            }
                            return request.isForMainFrame
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
}

package net.obscura.vpnclientapp.ui.commands

import android.content.Context
import android.content.Intent
import android.os.Handler
import android.os.Looper
import net.obscura.vpnclientapp.sharing.DebugBundleFileProvider

fun shareDebugBundle(
    context: Context,
    path: String,
    email: Boolean,
    subject: String? = null,
    body: String? = null,
) =
    DebugBundleFileProvider.shareFile(context, path).thenAccept { fileUri ->
      Handler(Looper.getMainLooper()).post {
        context.startActivity(
            Intent(Intent.ACTION_SEND).apply {
              putExtra(
                  Intent.EXTRA_STREAM,
                  fileUri,
              )

              if (email) {
                setType("message/rfc822")
                putExtra(Intent.EXTRA_EMAIL, arrayOf("support@obscura.net"))
                putExtra(
                    Intent.EXTRA_SUBJECT,
                    subject,
                )
                putExtra(
                    Intent.EXTRA_TEXT,
                    body,
                )
              } else {
                setType("application/zip")
              }
            },
        )
      }
    }

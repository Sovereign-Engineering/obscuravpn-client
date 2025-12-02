package net.obscura.vpnclientapp.sharing

import android.content.Context
import androidx.core.content.FileProvider
import java.io.File
import java.nio.file.Files.move
import java.nio.file.StandardCopyOption
import java.util.concurrent.CompletableFuture
import net.obscura.vpnclientapp.R
import net.obscura.vpnclientapp.helpers.currentApp

/**
 * Makes files available for sharing. These files only live in the cache directory so they will be
 * automatically cleaned up by Android when disk space is running low.
 */
class DebugBundleFileProvider : FileProvider(R.xml.debug_bundle_file_provider) {
  companion object {
    fun shareFile(
        context: Context,
        path: String,
    ) =
        CompletableFuture.supplyAsync(
            {
              val originalFile = File(path)
              val sharedFile = File(File(context.cacheDir, "debug_bundle"), originalFile.name)

              sharedFile.mkdirs()
              move(
                  originalFile.toPath(),
                  sharedFile.toPath(),
                  StandardCopyOption.REPLACE_EXISTING,
              )

              getUriForFile(
                  context,
                  "net.obscura.vpnclientapp.debug_bundle_file_provider",
                  sharedFile,
              )
            },
            context.currentApp().ioExecutor,
        )
  }
}

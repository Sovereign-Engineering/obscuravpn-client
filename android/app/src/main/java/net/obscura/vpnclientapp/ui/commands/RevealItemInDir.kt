package net.obscura.vpnclientapp.ui.commands

import android.content.Context
import java.util.concurrent.CompletableFuture
import kotlinx.serialization.Serializable

@Serializable
data class RevealItemInDir(
    val path: String,
) {
  fun run(context: Context) =
      CompletableFuture<String>().thenRun {
        throw RuntimeException("revealItemInDir command is used only in desktop apps")
      }
}

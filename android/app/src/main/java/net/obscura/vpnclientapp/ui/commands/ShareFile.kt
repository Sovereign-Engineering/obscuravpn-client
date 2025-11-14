package net.obscura.vpnclientapp.ui.commands

import android.content.Context
import kotlinx.serialization.Serializable

@Serializable
data class ShareFile(
    val path: String,
) {
  fun run(context: Context) = shareDebugBundle(context, path, false)
}

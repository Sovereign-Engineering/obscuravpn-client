package net.obscura.vpnclientapp.ui.commands

import android.content.Context
import kotlinx.serialization.Serializable

@Serializable
data class ShareArchive(
    val path: String,
) {
  fun run(context: Context) = shareDebugArchive(context, path, false)
}

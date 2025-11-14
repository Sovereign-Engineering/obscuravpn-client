package net.obscura.vpnclientapp.ui.commands

import android.content.Context
import kotlinx.serialization.Serializable

@Serializable
data class EmailArchive(
    val path: String,
    val subject: String,
    val body: String,
) {
  fun run(context: Context) = shareDebugBundle(context, path, true, subject, body)
}

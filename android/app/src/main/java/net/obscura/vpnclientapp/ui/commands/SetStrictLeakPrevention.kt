package net.obscura.vpnclientapp.ui.commands

import android.content.Context
import kotlinx.serialization.Serializable
import net.obscura.vpnclientapp.preferences.Preferences

@Serializable
data class SetStrictLeakPrevention(
    val enable: Boolean,
) {
  fun run(context: Context) {
    Preferences(context).strictLeakPrevention = enable
  }
}

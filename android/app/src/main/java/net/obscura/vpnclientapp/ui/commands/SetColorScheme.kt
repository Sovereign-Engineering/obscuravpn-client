package net.obscura.vpnclientapp.ui.commands

import android.content.Context
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import net.obscura.vpnclientapp.preferences.Preferences

@Serializable
data class SetColorScheme(
    val value: ColorScheme,
) {
  @Serializable
  enum class ColorScheme {
    @SerialName("dark") Dark,
    @SerialName("light") Light,
    @SerialName("auto") Auto,
  }

  fun run(context: Context) {
    Preferences(context).colorScheme = value
  }
}

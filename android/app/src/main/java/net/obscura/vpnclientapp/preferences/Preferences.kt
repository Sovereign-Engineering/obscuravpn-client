package net.obscura.vpnclientapp.preferences

import android.content.Context
import android.content.SharedPreferences
import androidx.core.content.edit
import kotlinx.serialization.json.Json
import net.obscura.vpnclientapp.ui.commands.SetColorScheme

class Preferences(
    context: Context,
) {
  private val preferences = context.getSharedPreferences("preferences", Context.MODE_PRIVATE)

  var colorScheme: SetColorScheme.ColorScheme
    get() =
        Json.decodeFromString<SetColorScheme.ColorScheme>(
            preferences.getString(
                "color-scheme",
                "\"auto\"",
            )!!,
        )
    set(value) {
      preferences.edit(commit = true) {
        putString(
            "color-scheme",
            Json.encodeToString(value),
        )
      }
    }

  var permissionGiven: Boolean
    get() = preferences.getBoolean("permission-given", false)
    set(value) = preferences.edit { putBoolean("permission-given", value) }

  fun registerListener(listener: SharedPreferences.OnSharedPreferenceChangeListener) {
    preferences.registerOnSharedPreferenceChangeListener(listener)
  }

  fun unregisterListener(listener: SharedPreferences.OnSharedPreferenceChangeListener) {
    preferences.unregisterOnSharedPreferenceChangeListener(listener)
  }
}

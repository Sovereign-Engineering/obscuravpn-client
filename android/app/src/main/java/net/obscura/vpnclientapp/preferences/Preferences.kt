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
      preferences.edit(commit = true) { putString("color-scheme", Json.encodeToString(value)) }
    }

  // TODO: https://linear.app/soveng/issue/OBS-2639/store-strict-leak-prevention-in-core Should be
  // handled by core / Rust library.
  var strictLeakPrevention: Boolean
    get() = preferences.getBoolean("strict-leak-prevention", false)
    set(value) = preferences.edit { putBoolean("strict-leak-prevention", value) }

  fun registerListener(listener: SharedPreferences.OnSharedPreferenceChangeListener) {
    preferences.registerOnSharedPreferenceChangeListener(listener)
  }

  fun unregisterListener(listener: SharedPreferences.OnSharedPreferenceChangeListener) {
    preferences.unregisterOnSharedPreferenceChangeListener(listener)
  }
}

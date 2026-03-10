package net.obscura.vpnclientapp.preferences

import android.content.Context
import android.content.SharedPreferences
import androidx.core.content.edit
import kotlinx.serialization.json.Json
import net.obscura.vpnclientapp.helpers.requireUIProcess
import net.obscura.vpnclientapp.ui.commands.SetColorScheme

class Preferences(context: Context) {
    init {
        requireUIProcess()
    }

    private val sharedPreferences = context.getSharedPreferences("preferences", Context.MODE_PRIVATE)

    var colorScheme: SetColorScheme.ColorScheme
        get() =
            Json.decodeFromString<SetColorScheme.ColorScheme>(
                this.sharedPreferences.getString("color-scheme", "\"auto\"")!!
            )
        set(value) {
            this.sharedPreferences.edit(commit = true) { putString("color-scheme", Json.encodeToString(value)) }
        }

    fun registerListener(listener: SharedPreferences.OnSharedPreferenceChangeListener) {
        this.sharedPreferences.registerOnSharedPreferenceChangeListener(listener)
    }

    fun unregisterListener(listener: SharedPreferences.OnSharedPreferenceChangeListener) {
        this.sharedPreferences.unregisterOnSharedPreferenceChangeListener(listener)
    }
}

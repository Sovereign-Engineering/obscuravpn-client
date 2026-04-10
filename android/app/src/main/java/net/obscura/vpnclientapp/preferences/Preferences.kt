package net.obscura.vpnclientapp.preferences

import android.content.Context
import android.content.SharedPreferences
import androidx.core.content.edit
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import net.obscura.vpnclientapp.client.jsonConfig
import net.obscura.vpnclientapp.helpers.requireUIProcess

class Preferences(context: Context) {
    init {
        requireUIProcess()
    }

    @Serializable
    enum class ColorScheme {
        @SerialName("dark") Dark,
        @SerialName("light") Light,
        @SerialName("auto") Auto,
    }

    private val sharedPreferences = context.getSharedPreferences("preferences", Context.MODE_PRIVATE)

    var colorScheme: ColorScheme
        get() = jsonConfig.decodeFromString<ColorScheme>(this.sharedPreferences.getString("color-scheme", "\"auto\"")!!)
        set(value) {
            this.sharedPreferences.edit(commit = true) { putString("color-scheme", jsonConfig.encodeToString(value)) }
        }

    fun registerListener(listener: SharedPreferences.OnSharedPreferenceChangeListener) {
        this.sharedPreferences.registerOnSharedPreferenceChangeListener(listener)
    }

    fun unregisterListener(listener: SharedPreferences.OnSharedPreferenceChangeListener) {
        this.sharedPreferences.unregisterOnSharedPreferenceChangeListener(listener)
    }
}

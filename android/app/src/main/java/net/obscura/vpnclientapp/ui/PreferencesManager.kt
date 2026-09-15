package net.obscura.vpnclientapp.ui

import android.content.Context
import android.content.SharedPreferences
import androidx.core.content.edit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.client.jsonConfig

private val log = Logger(PreferencesManager::class)

class PreferencesManager(context: Context, private val onPreferencesChanged: (Preferences) -> Unit) {
    data class Preferences(val colorScheme: OsStatus.ColorScheme) {
        companion object {
            private const val KEY_COLOR_SCHEME = "color-scheme"

            internal fun decode(sharedPreferences: SharedPreferences?): Preferences {
                val colorScheme =
                    runCatching {
                            sharedPreferences?.getString(KEY_COLOR_SCHEME, null)?.let {
                                jsonConfig.decodeFromString<OsStatus.ColorScheme>(it)
                            }
                        }
                        .onFailure { log.error("failed to read ${KEY_COLOR_SCHEME}: ${it.message}") }
                        .getOrNull() ?: OsStatus.ColorScheme.Auto
                return Preferences(colorScheme)
            }
        }

        internal fun encode(editor: SharedPreferences.Editor) {
            editor.putString(KEY_COLOR_SCHEME, jsonConfig.encodeToString(this.colorScheme))
        }
    }

    // This is disk I/O, but since `uiMode` configuration changes lead to activity recreation, it's essential to wait
    // on this before finishing activity creation.
    private val sharedPreferences = context.getSharedPreferences("preferences", Context.MODE_PRIVATE)
    private val sharedPreferencesEditorLock = Mutex()

    init {
        this.onPreferencesChanged(Preferences.decode(this.sharedPreferences))
    }

    suspend fun write(preferences: Preferences) {
        this.sharedPreferencesEditorLock.withLock {
            // Shared preference read/writes are disk I/O
            withContext(Dispatchers.IO) {
                this@PreferencesManager.sharedPreferences.edit(commit = true) { preferences.encode(this) }
            }
            this.onPreferencesChanged(preferences)
        }
    }
}

package net.obscura.vpnclientapp

import android.app.Application
import dagger.hilt.android.HiltAndroidApp
import net.obscura.lib.util.Logger

private val log = Logger(App::class)

@HiltAndroidApp
class App : Application() {
    override fun onCreate() {
        super.onCreate()
        log.info("app version: ${BuildConfig.VERSION_NAME}")
    }
}

package net.obscura.vpnclientapp

import android.app.Application
import net.obscura.lib.util.Logger

private val log = Logger(App::class)

class App : Application() {
    override fun onCreate() {
        super.onCreate()
        log.info("app version: ${BuildConfig.VERSION_NAME}")
    }
}

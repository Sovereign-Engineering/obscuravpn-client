package net.obscura.vpnclientapp

import android.app.Application
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.client.ObscuraLibrary

private val log = Logger(App::class)

class App : Application() {
    override fun onCreate() {
        super.onCreate()

        val userAgent = "obscura.net/android/${BuildConfig.VERSION_NAME}"
        log.info("user agent: $userAgent")
        ObscuraLibrary.load(this, userAgent)
    }
}

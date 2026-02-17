package net.obscura.vpnclientapp

import android.app.Application
import net.obscura.vpnclientapp.client.ObscuraLibrary
import net.obscura.vpnclientapp.helpers.logInfo

class App : Application() {
    override fun onCreate() {
        super.onCreate()

        val userAgent = "obscura.net/android/${BuildConfig.VERSION_NAME}"
        logInfo("user agent: $userAgent")
        ObscuraLibrary.load(this, userAgent)
    }
}

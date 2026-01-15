package net.obscura.vpnclientapp

import android.app.Application
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import net.obscura.vpnclientapp.BuildConfig
import net.obscura.vpnclientapp.client.ObscuraLibrary
import net.obscura.vpnclientapp.helpers.logInfo

class App : Application() {
  private lateinit var _ioExecutor: ExecutorService

  val ioExecutor: ExecutorService
    get() = _ioExecutor

  override fun onCreate() {
    super.onCreate()

    val userAgent = "obscura.net/android/${BuildConfig.VERSION_NAME}"
    logInfo("user agent: $userAgent")
    ObscuraLibrary.load(this, userAgent)

    _ioExecutor = Executors.newCachedThreadPool()
  }
}

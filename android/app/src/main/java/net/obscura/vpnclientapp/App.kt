package net.obscura.vpnclientapp

import android.app.Application
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import net.obscura.vpnclientapp.client.ObscuraLibrary
import net.obscura.vpnclientapp.ui.OsStatus

class App : Application() {
  private lateinit var _osStatus: OsStatus

  val osStatus: OsStatus
    get() = _osStatus

  private lateinit var _ioExecutor: ExecutorService

  val ioExecutor: ExecutorService
    get() = _ioExecutor

  override fun onCreate() {
    super.onCreate()

    _osStatus = OsStatus(this)
    _ioExecutor = Executors.newCachedThreadPool()

    osStatus.registerCallbacks()

    ObscuraLibrary.initialize(filesDir.path, "Obscura Android TODO")
  }
}

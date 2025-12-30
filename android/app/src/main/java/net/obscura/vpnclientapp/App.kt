package net.obscura.vpnclientapp

import android.app.Application
import java.util.concurrent.ExecutorService
import java.util.concurrent.Executors
import net.obscura.vpnclientapp.client.ObscuraLibrary

class App : Application() {
  private lateinit var _ioExecutor: ExecutorService

  val ioExecutor: ExecutorService
    get() = _ioExecutor

  override fun onCreate() {
    super.onCreate()

    ObscuraLibrary.load(this)

    _ioExecutor = Executors.newCachedThreadPool()
  }
}

package net.obscura.vpnclientapp.helpers

import android.app.Application

/** Ensures the calling process is :vpnservice. */
fun requireVpnServiceProcess() {
  val currentProcess = Application.getProcessName()

  if (!currentProcess.endsWith(":vpnservice")) {
    throw RuntimeException("Called outside of the :vpnservice process ($currentProcess)")
  }
}

/** Ensures the calling process is the main application process. */
fun requireUIProcess() {
  val currentProcess = Application.getProcessName()

  if (currentProcess.contains(":")) {
    throw RuntimeException("Called outside of the application process ($currentProcess)")
  }
}

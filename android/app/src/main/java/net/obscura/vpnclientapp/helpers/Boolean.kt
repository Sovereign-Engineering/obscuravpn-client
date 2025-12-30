package net.obscura.vpnclientapp.helpers

inline fun Boolean.whenTrue(crossinline block: () -> Unit): Boolean {
  if (this) {
    block()
  }

  return this
}

package net.obscura.vpnclientapp.helpers

import android.net.Uri

fun Uri.alwaysHTTPS(): Uri {
  if (scheme == "http") {
    return buildUpon().scheme("https").build()
  }

  return this
}

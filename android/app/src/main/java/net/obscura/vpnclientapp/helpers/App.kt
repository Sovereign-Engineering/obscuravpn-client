package net.obscura.vpnclientapp.helpers

import android.content.Context
import net.obscura.vpnclientapp.App

inline fun Context.currentApp(): App = applicationContext as App

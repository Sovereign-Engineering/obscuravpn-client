package net.obscura.vpnclientapp

import android.app.Application
import android.os.Build
import android.os.StrictMode
import android.os.strictmode.DiskReadViolation
import android.os.strictmode.ExplicitGcViolation
import android.os.strictmode.InstanceCountViolation
import android.os.strictmode.UntaggedSocketViolation
import dagger.hilt.android.HiltAndroidApp
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.asExecutor
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.ObscuraLibrary

private val log = Logger(App::class)

@HiltAndroidApp
class App : Application() {
    override fun onCreate() {
        super.onCreate()
        log.info("app version: ${BuildConfig.VERSION_NAME}")
        if (BuildConfig.DEBUG) {
            val executor = Dispatchers.Default.asExecutor()
            val strictModeLog = Logger(StrictMode::class)
            StrictMode.setThreadPolicy(
                StrictMode.ThreadPolicy.Builder()
                    .detectAll()
                    .penaltyListener(executor) { violation ->
                        // `MainActivity` creation intentionally depends on reading shared preferences.
                        if (
                            violation is DiskReadViolation &&
                                violation.stackTrace.any {
                                    it.className == MainActivity::class.java.name && it.methodName == "onCreate"
                                }
                        )
                            return@penaltyListener
                        // The VPN service creation intentionally depends on the native library.
                        if (
                            violation is DiskReadViolation &&
                                violation.stackTrace.any { it.className == ObscuraLibrary::class.java.name }
                        )
                            return@penaltyListener
                        // Android appears to violate its own strict mode.
                        // Also observed by this project:
                        // https://github.com/nice-devone/nice-cxone-mobile-sdk-android/blob/67d07af4555a03ea63537dd2085485ad8c2f0df3/store/src/main/kotlin/com/nice/cxonechat/sample/StrictModePolicy.kt#L251-L263
                        if (
                            Build.VERSION.SDK_INT >= Build.VERSION_CODES.UPSIDE_DOWN_CAKE &&
                                violation is ExplicitGcViolation &&
                                violation.stackTrace.any {
                                    it.className == "android.app.ActivityThread" &&
                                        it.methodName == "performDestroyActivity"
                                }
                        )
                            return@penaltyListener
                        strictModeLog.warn("thread policy violation", tr = violation)
                    }
                    .build()
            )
            StrictMode.setVmPolicy(
                StrictMode.VmPolicy.Builder()
                    .detectAll()
                    .penaltyListener(executor) { violation ->
                        // False positive on activity recreation: https://stackoverflow.com/a/24617415
                        // There's nothing in the stacktrace to let us constrain this to `MainActivity`.
                        if (violation is InstanceCountViolation) return@penaltyListener
                        // The default tag of 0 is fine for us:
                        // https://source.android.com/docs/core/data/tags-explained
                        if (violation is UntaggedSocketViolation) return@penaltyListener
                        strictModeLog.warn("VM policy violation", tr = violation)
                    }
                    .build()
            )
        }
    }
}

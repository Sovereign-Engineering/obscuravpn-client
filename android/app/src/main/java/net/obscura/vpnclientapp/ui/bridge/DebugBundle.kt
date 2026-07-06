package net.obscura.vpnclientapp.ui.bridge

import android.app.ActivityManager
import android.app.Application
import android.content.Context
import android.os.Build
import android.os.PowerManager
import android.os.Process
import android.os.SystemClock
import kotlin.time.Instant
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.BuildConfig
import net.obscura.vpnclientapp.client.ManagerCmd

private val log = Logger("DebugBundle")

fun bundleInfo(context: Context): ManagerCmd.CreateDebugBundle.BundleInfo {
    val memoryInfo =
        runCatching {
                val activityManager = context.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
                val memoryInfo = ActivityManager.MemoryInfo()
                activityManager.getMemoryInfo(memoryInfo)
                memoryInfo
            }
            .onFailure { log.error("failed to get memory info: ${it.message}") }
            .getOrNull()
    val powerManager =
        runCatching { context.getSystemService(Context.POWER_SERVICE) as PowerManager }
            .onFailure { log.error("failed to get power manager: ${it.message}") }
            .getOrNull()
    val uptimeMs = SystemClock.elapsedRealtime()
    return ManagerCmd.CreateDebugBundle.BundleInfo(
            androidSdk = Build.VERSION.SDK_INT,
            appVersion = BuildConfig.VERSION_NAME,
            bootTimestamp = Instant.fromEpochMilliseconds(System.currentTimeMillis() - uptimeMs).toString(),
            brand = Build.BRAND,
            lowPowerMode = powerManager?.isPowerSaveMode,
            memoryAvailGib = memoryInfo?.let { it.availMem.toDouble() / 1024.0 / 1024.0 / 1024.0 },
            memoryTotalGib = memoryInfo?.let { it.totalMem.toDouble() / 1024.0 / 1024.0 / 1024.0 },
            model = Build.MODEL,
            osVersion = Build.VERSION.RELEASE_OR_CODENAME,
            processId = Process.myPid(),
            processName = Application.getProcessName(),
            processorCountActive = Runtime.getRuntime().availableProcessors(),
            processorName = Build.SOC_MODEL,
            thermalState =
                powerManager?.currentThermalStatus?.let {
                    when (it) {
                        PowerManager.THERMAL_STATUS_NONE -> "none"
                        PowerManager.THERMAL_STATUS_LIGHT -> "light"
                        PowerManager.THERMAL_STATUS_MODERATE -> "moderate"
                        PowerManager.THERMAL_STATUS_SEVERE -> "severe"
                        PowerManager.THERMAL_STATUS_CRITICAL -> "critical"
                        PowerManager.THERMAL_STATUS_EMERGENCY -> "emergency"
                        PowerManager.THERMAL_STATUS_SHUTDOWN -> "shutdown"
                        else -> it.toString()
                    }
                },
            uptimeHours = uptimeMs.toDouble() / 3_600_000.0,
        )
        .also { log.debug("bundle info: $it") }
}

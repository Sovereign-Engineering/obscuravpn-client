package net.obscura.vpnclientapp.services

import android.content.Context
import android.content.Context.BIND_AUTO_CREATE
import android.content.Intent
import android.content.ServiceConnection
import android.net.VpnService.prepare
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.client.errorCodeOther

private val log = Logger("ContextExtension")

fun Context.bindVpnService(serviceConnection: ServiceConnection): Boolean {
    log.info("binding VPN service")
    val intent = Intent(this, ObscuraVpnService::class.java)
    return try {
        val isBinding = this.bindService(intent, serviceConnection, BIND_AUTO_CREATE)
        if (!isBinding) {
            log.error("missing permissions or service not found")
        }
        isBinding
    } catch (e: SecurityException) {
        log.error("missing permissions or service not found", tr = e)
        this.unbindVpnService(serviceConnection)
        false
    }
}

fun Context.unbindVpnService(serviceConnection: ServiceConnection) {
    log.info("unbinding VPN service")
    try {
        this.unbindService(serviceConnection)
    } catch (e: IllegalArgumentException) {
        log.error("VPN service connection not registered", tr = e)
    }
}

sealed interface PrepareResult {
    data class CreateProfile(val intent: Intent) : PrepareResult

    data object Ready : PrepareResult

    data object LegacyAlwaysOn : PrepareResult
}

fun Context.prepareVpnService(): PrepareResult =
    try {
        log.info("preparing VPN service")
        prepare(this)?.let { PrepareResult.CreateProfile(it) } ?: PrepareResult.Ready
    } catch (e: IllegalStateException) {
        // This is undocumented, but `prepare` throws when a Legacy VPN is set to Always-On.
        // Legacy VPN profiles are created manually using the "+" button on "Network & Internet" -> "VPN".
        // https://cs.android.com/android/platform/superproject/+/android-latest-release:frameworks/base/services/core/java/com/android/server/VpnManagerService.java;l=226;drc=0b5a5f8c78ce8e8800b527216b70db35489b7c41
        // https://cs.android.com/android/platform/superproject/+/android-latest-release:frameworks/base/services/core/java/com/android/server/VpnManagerService.java;l=545-557;drc=0b5a5f8c78ce8e8800b527216b70db35489b7c41
        log.error("a Legacy VPN profile is set to Always-On", tr = e)
        PrepareResult.LegacyAlwaysOn
    }

fun Context.startVpnService(): Result<Unit> =
    try {
        log.info("starting VPN service")
        this.startForegroundService(Intent(this, ObscuraVpnService::class.java))
        Result.success(Unit)
    } catch (e: SecurityException) {
        log.error("missing permissions or service not found", tr = e)
        Result.failure(errorCodeOther())
    } catch (e: IllegalStateException) {
        log.error("app not foregrounded", tr = e)
        Result.failure(errorCodeOther())
    }

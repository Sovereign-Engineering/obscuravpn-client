package net.obscura.vpnclientapp.ui

import android.Manifest
import android.app.Activity.RESULT_CANCELED
import android.app.Activity.RESULT_OK
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import androidx.activity.result.ActivityResult
import androidx.activity.result.ActivityResultLauncher
import androidx.activity.result.contract.ActivityResultContracts
import androidx.core.content.ContextCompat
import androidx.fragment.app.FragmentActivity
import dagger.hilt.android.scopes.ActivityScoped
import javax.inject.Inject
import kotlin.time.Duration
import kotlin.time.Duration.Companion.milliseconds
import kotlin.time.TimeSource
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import kotlinx.coroutines.flow.firstOrNull
import kotlinx.coroutines.flow.onSubscription
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.client.errorCodeLegacyAlwaysOn
import net.obscura.vpnclientapp.client.errorCodeOther
import net.obscura.vpnclientapp.client.errorCodeOtherAppAlwaysOn
import net.obscura.vpnclientapp.client.errorCodePermissionNotGranted
import net.obscura.vpnclientapp.services.PrepareResult
import net.obscura.vpnclientapp.services.prepareVpnService
import net.obscura.vpnclientapp.services.startVpnService

private val log = Logger(VpnPermissionRequestManager::class)

@ActivityScoped
class VpnPermissionRequestManager @Inject constructor(private val activity: FragmentActivity) {
    private val vpnPermissionRequestCancelThreshold: Duration = 150.milliseconds

    private val vpnPermissionRequestResultTx = MutableSharedFlow<ActivityResult>(extraBufferCapacity = 1)
    private val vpnPermissionRequestResultRx = this.vpnPermissionRequestResultTx.asSharedFlow()

    private val vpnPermissionRequestLauncher: ActivityResultLauncher<Intent> =
        this.activity.registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
            log.debug("VPN permission request activity result: $result")
            val wasEmitted = this.vpnPermissionRequestResultTx.tryEmit(result)
            if (!wasEmitted) {
                log.warn("multiple VPN permission requests while collecting")
            }
        }

    private val notificationPermissionRequestLauncher: ActivityResultLauncher<String> =
        this.activity.registerForActivityResult(ActivityResultContracts.RequestPermission()) { isGranted ->
            // We don't actually care if we're granted permission, since this is
            // just the user's preference between "classic" foreground service
            // notifications vs. the modern Task Manager.
            log.debug("notification permission request activity result: $isGranted")
        }

    private fun requestNotificationPermission() {
        if (
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
                ContextCompat.checkSelfPermission(this.activity, Manifest.permission.POST_NOTIFICATIONS) !=
                    PackageManager.PERMISSION_GRANTED
        ) {
            this.notificationPermissionRequestLauncher.launch(Manifest.permission.POST_NOTIFICATIONS)
        }
    }

    private fun onSuccess(): Result<Unit> {
        this.requestNotificationPermission()
        return this.activity.startVpnService()
    }

    // Android 12+ has no API for checking if another app has Always-On enabled. Instead, the permission request
    // receives `RESULT_CANCELED` immediately, requiring us to use a heuristic to determine that we're silently
    // unable to request VPN permissions from the user.
    private fun onCanceled(vpnPermissionRequestStart: TimeSource.Monotonic.ValueTimeMark): Result<Unit> {
        val vpnPermissionRequestEnd = TimeSource.Monotonic.markNow()
        val elapsed = vpnPermissionRequestEnd - vpnPermissionRequestStart
        log.debug("$elapsed elapsed between VPN permission request launch and cancellation")
        return if (elapsed > this.vpnPermissionRequestCancelThreshold) {
            log.debug("heuristic determined that cancellation was user-initiated")
            Result.failure(errorCodePermissionNotGranted())
        } else {
            log.debug("heuristic determined that cancellation was automatic")
            Result.failure(errorCodeOtherAppAlwaysOn())
        }
    }

    suspend fun requestVpnStart(): Result<Unit> =
        when (val prepareResult = this.activity.prepareVpnService()) {
            is PrepareResult.CreateProfile -> {
                val vpnPermissionRequestStart = TimeSource.Monotonic.markNow()
                val vpnPermissionRequestResult =
                    this.vpnPermissionRequestResultRx
                        .onSubscription {
                            this@VpnPermissionRequestManager.vpnPermissionRequestLauncher.launch(prepareResult.intent)
                        }
                        .firstOrNull()
                        ?: run {
                            log.error("VPN permission request result flow was empty")
                            return Result.failure(errorCodeOther())
                        }
                when (vpnPermissionRequestResult.resultCode) {
                    RESULT_OK -> this.onSuccess()
                    RESULT_CANCELED -> this.onCanceled(vpnPermissionRequestStart)
                    else -> {
                        log.error("unexpected VPN start activity result: $vpnPermissionRequestResult")
                        Result.failure(errorCodeOther())
                    }
                }
            }
            is PrepareResult.Ready -> this.onSuccess()
            is PrepareResult.LegacyAlwaysOn -> Result.failure(errorCodeLegacyAlwaysOn())
        }
}

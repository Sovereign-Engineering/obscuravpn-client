package net.obscura.vpnclientapp.services

import android.Manifest
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import androidx.core.app.NotificationChannelCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import androidx.core.content.ContextCompat
import androidx.core.net.toUri
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.BuildConfig
import net.obscura.vpnclientapp.R
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.ManagerCmdOk
import net.obscura.vpnclientapp.ui.OsStatus

private val log = Logger(NotificationManager::class)

internal class NotificationManager(private val context: Context) {
    companion object {
        private const val NOTIFICATION_CHANNEL_ID = "vpn_channel"
        const val NOTIFICATION_ID = 1
        private const val PENDING_INTENT_FLAGS = PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT
    }

    private val manager = NotificationManagerCompat.from(this.context)

    init {
        this.manager.createNotificationChannel(
            NotificationChannelCompat.Builder(
                    NOTIFICATION_CHANNEL_ID,
                    NotificationManagerCompat.IMPORTANCE_LOW,
                )
                .setName(this.context.getString(R.string.notification_channel_vpn_name))
                .build(),
        )
    }

    private fun buildIntentMainActivity(path: String?) =
        PendingIntent.getActivity(
            this.context,
            0,
            Intent().apply {
                this.action = Intent.ACTION_MAIN
                path?.let { this.data = "/$it".toUri() }
                this.flags = Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP
                this.setClassName(
                    BuildConfig.APPLICATION_ID,
                    MainActivity::class.qualifiedName!!,
                )
            },
            PENDING_INTENT_FLAGS,
        )

    private fun buildIntentVpnService(action: String) =
        // `getForegroundService` would be functionally equivalent, but leads to an ANR if the action is stale and VPN
        // permission was revoked. While this initially starts the service in the background, `VpnService` is allowed
        // to be started in the background on a temporary basis:
        // https://developer.android.com/about/versions/oreo/background#services
        PendingIntent.getService(
            this.context,
            0,
            Intent().apply {
                this.action = action
                this.setClassName(
                    BuildConfig.APPLICATION_ID,
                    ObscuraVpnService::class.qualifiedName!!,
                )
            },
            PENDING_INTENT_FLAGS,
        )

    private fun buildAction(resId: Int, pendingIntent: PendingIntent) =
        NotificationCompat.Action.Builder(
                null, // The icon argument has been ignored since Android 26.
                this.context.getString(resId),
                pendingIntent,
            )
            .build()

    private fun buildConnectionAction(vpnStatus: ManagerCmdOk.GetStatus.VpnStatus?, prepareResult: PrepareResult) =
        when (vpnStatus) {
            is ManagerCmdOk.GetStatus.VpnStatus.Connected,
            is ManagerCmdOk.GetStatus.VpnStatus.Connecting,
            ->
                this.buildAction(
                    R.string.notification_action_disconnect,
                    this.buildIntentVpnService(ACTION_STOP_TUNNEL),
                )
            is ManagerCmdOk.GetStatus.VpnStatus.Disconnected,
            null ->
                this.buildAction(
                    R.string.notification_action_quick_connect,
                    when (prepareResult) {
                        PrepareResult.Ready -> this.buildIntentVpnService(ACTION_START_TUNNEL)
                        else -> this.buildIntentMainActivity(PATH_REQUEST_VPN_START)
                    },
                )
        }

    private fun buildLocationAction() =
        this.buildAction(
            R.string.notification_action_location,
            this.buildIntentMainActivity(OsStatus.NavigationView.Location.serialName()),
        )

    fun buildNotification(vpnStatus: ManagerCmdOk.GetStatus.VpnStatus?, prepareResult: PrepareResult) =
        NotificationCompat.Builder(this.context, NOTIFICATION_CHANNEL_ID)
            .addAction(this.buildConnectionAction(vpnStatus, prepareResult))
            .addAction(this.buildLocationAction())
            .setContentIntent(this.buildIntentMainActivity(null))
            .setContentTitle(this.context.getString(R.string.app_name))
            .setContentText(
                this.context.getString(
                    R.string.notification_vpn_text,
                    when (vpnStatus) {
                        is ManagerCmdOk.GetStatus.VpnStatus.Connected ->
                            this.context.getString(R.string.notification_vpn_status_connected)
                        is ManagerCmdOk.GetStatus.VpnStatus.Connecting ->
                            this.context.getString(R.string.notification_vpn_status_connecting)
                        is ManagerCmdOk.GetStatus.VpnStatus.Disconnected,
                        null -> this.context.getString(R.string.notification_vpn_status_disconnected)
                    },
                ),
            )
            .setSmallIcon(R.drawable.ic_stat_name)
            .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
            .setOngoing(true)
            .setLocalOnly(true)
            .setOnlyAlertOnce(true)
            .setCategory(NotificationCompat.CATEGORY_SERVICE)
            .build()

    fun notify(vpnStatus: ManagerCmdOk.GetStatus.VpnStatus?, prepareResult: PrepareResult) =
        runCatching {
                // Defensive permission check in case user revokes permissions.
                if (
                    ContextCompat.checkSelfPermission(
                        this.context,
                        Manifest.permission.POST_NOTIFICATIONS,
                    ) == PackageManager.PERMISSION_GRANTED
                ) {
                    this.manager.notify(
                        NOTIFICATION_ID,
                        this.buildNotification(vpnStatus, prepareResult),
                    )
                }
            }
            .onFailure { log.error("failed to update notification: ${it.message}") }
}

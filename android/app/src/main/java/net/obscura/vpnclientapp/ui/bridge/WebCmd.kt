package net.obscura.vpnclientapp.ui.bridge

import android.content.Context
import kotlinx.serialization.KeepGeneratedSerializer
import kotlinx.serialization.Serializable
import net.obscura.lib.util.ExternallyTaggedEnumSerializer
import net.obscura.lib.util.ExternallyTaggedEnumVariantSerializer
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.ManagerCmd
import net.obscura.vpnclientapp.client.errorCodeUnsupportedOnOS
import net.obscura.vpnclientapp.client.jsonConfig
import net.obscura.vpnclientapp.preferences.Preferences
import net.obscura.vpnclientapp.services.IObscuraVpnService
import net.obscura.vpnclientapp.ui.JsonFfiBroadcastReceiver
import net.obscura.vpnclientapp.ui.OsStatusManager

private val jsonUnit = jsonConfig.encodeToString(Unit)

@Serializable(with = WebCmd.Serializer::class)
internal sealed interface WebCmd {
    object Serializer :
        ExternallyTaggedEnumSerializer<WebCmd>(
            WebCmd::class,
            listOf(
                DebuggingArchive.Serializer,
                EmailDebugArchive.Serializer,
                GetOsStatus.Serializer,
                JsonFfiCmd.Serializer,
                PurchaseSubscription.Serializer,
                RevealItemInDir.Serializer,
                SetColorScheme.Serializer,
                SetFeatureFlag.Serializer,
                ShareDebugArchive.Serializer,
                StartTunnel.Serializer,
                StopTunnel.Serializer,
            ),
        )

    data class Args(
        val context: Context,
        val binder: IObscuraVpnService,
        val mainActivity: MainActivity,
        val osStatusManager: OsStatusManager,
    )

    suspend fun run(args: Args): String

    @KeepGeneratedSerializer
    @Serializable(with = DebuggingArchive.Serializer::class)
    data class DebuggingArchive(
        val userFeedback: String?,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<DebuggingArchive>("debuggingArchive", generatedSerializer())

        // Eventually, all platforms should just use the JSON FFI command to create debug archives, but for now,
        // adapting the command here is the least invasive change.
        // TODO: https://linear.app/soveng/issue/OBS-3095/cross-platform-debug-archive-story
        override suspend fun run(args: Args) =
            jsonUnit.also {
                args.osStatusManager.update { this.debugBundleStatus.inProgress = true }
                val path = runCatching {
                    JsonFfiCmd(jsonConfig.encodeToString(ManagerCmd.CreateDebugArchive(userFeedback))).run(args).let {
                        jsonConfig.decodeFromString<String>(it)
                    }
                }
                args.osStatusManager.update {
                    this.debugBundleStatus.inProgress = false
                    path.onSuccess { this.debugBundleStatus.latestPath = it }
                }
                path.getOrThrow()
            }
    }

    @KeepGeneratedSerializer
    @Serializable(with = EmailDebugArchive.Serializer::class)
    data class EmailDebugArchive(
        val path: String,
        val subject: String,
        val body: String,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<EmailDebugArchive>("emailDebugArchive", generatedSerializer())

        override suspend fun run(args: Args) =
            jsonUnit.also { shareDebugArchive(args.context, path, true, subject, body) }
    }

    @KeepGeneratedSerializer
    @Serializable(with = GetOsStatus.Serializer::class)
    data class GetOsStatus(val knownVersion: String?) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<GetOsStatus>("getOsStatus", generatedSerializer())

        override suspend fun run(args: Args) = args.osStatusManager.waitForUpdate(knownVersion).await()
    }

    @KeepGeneratedSerializer
    @Serializable(with = JsonFfiCmd.Serializer::class)
    data class JsonFfiCmd(
        val cmd: String,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<JsonFfiCmd>("jsonFfiCmd", generatedSerializer())

        override suspend fun run(args: Args) = JsonFfiBroadcastReceiver.waitForResponse(args.binder, this.cmd).await()
    }

    @KeepGeneratedSerializer
    @Serializable(with = PurchaseSubscription.Serializer::class)
    class PurchaseSubscription : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<PurchaseSubscription>(
                "purchaseSubscription",
                generatedSerializer(),
            )

        override suspend fun run(args: Args) =
            args.mainActivity.billingFacade.launchFlow(args.mainActivity).let { jsonConfig.encodeToString(it) }
    }

    @KeepGeneratedSerializer
    @Serializable(with = RevealItemInDir.Serializer::class)
    data class RevealItemInDir(
        val path: String,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<RevealItemInDir>(
                "revealItemInDir",
                generatedSerializer(),
            )

        override suspend fun run(args: Args) = throw errorCodeUnsupportedOnOS()
    }

    @KeepGeneratedSerializer
    @Serializable(with = SetColorScheme.Serializer::class)
    data class SetColorScheme(
        val value: Preferences.ColorScheme,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<SetColorScheme>(
                "setColorScheme",
                generatedSerializer(),
            )

        override suspend fun run(args: Args) = jsonUnit.also { Preferences(args.context).colorScheme = this.value }
    }

    @KeepGeneratedSerializer
    @Serializable(with = SetFeatureFlag.Serializer::class)
    data class SetFeatureFlag(
        val flag: String,
        val active: Boolean,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<SetFeatureFlag>(
                "setFeatureFlag",
                generatedSerializer(),
            )

        override suspend fun run(args: Args) = throw errorCodeUnsupportedOnOS()
    }

    @KeepGeneratedSerializer
    @Serializable(with = ShareDebugArchive.Serializer::class)
    data class ShareDebugArchive(
        val path: String,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<ShareDebugArchive>(
                "shareDebugArchive",
                generatedSerializer(),
            )

        override suspend fun run(args: Args) = jsonUnit.also { shareDebugArchive(args.context, path, false) }
    }

    @KeepGeneratedSerializer
    @Serializable(with = StartTunnel.Serializer::class)
    data class StartTunnel(
        val tunnelArgs: String? = null,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<StartTunnel>(
                "startTunnel",
                generatedSerializer(),
            )

        override suspend fun run(args: Args) =
            jsonUnit.also {
                args.mainActivity.vpnPermissionRequestManager.requestVpnStart().getOrThrow()
                args.binder.startTunnel(this@StartTunnel.tunnelArgs)
            }
    }

    @KeepGeneratedSerializer
    @Serializable(with = StopTunnel.Serializer::class)
    class StopTunnel : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<StopTunnel>(
                "stopTunnel",
                generatedSerializer(),
            )

        override suspend fun run(args: Args) = jsonUnit.also { args.binder.stopTunnel() }
    }
}

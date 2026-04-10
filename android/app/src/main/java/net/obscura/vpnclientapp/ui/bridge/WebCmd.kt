package net.obscura.vpnclientapp.ui.bridge

import android.content.Context
import androidx.lifecycle.lifecycleScope
import java.util.concurrent.CompletableFuture
import kotlinx.coroutines.future.future
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
import net.obscura.vpnclientapp.ui.OsStatusManager

private fun completedVoid(): CompletableFuture<String> =
    CompletableFuture.completedFuture(jsonConfig.encodeToString(Unit))

private fun completedUnsupported(): CompletableFuture<String> =
    CompletableFuture<String>().apply { this.completeExceptionally(errorCodeUnsupportedOnOS()) }

@Serializable(with = WebCmd.Serializer::class)
internal sealed interface WebCmd {
    private object Serializer :
        ExternallyTaggedEnumSerializer<WebCmd>(
            WebCmd::class,
            listOf(
                DebuggingArchive.Serializer,
                EmailDebugArchive.Serializer,
                GetOsStatus.Serializer,
                JsonFfiCommand.Serializer,
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

    fun run(args: Args): CompletableFuture<String>

    @KeepGeneratedSerializer
    @Serializable(with = DebuggingArchive.Serializer::class)
    class DebuggingArchive(
        val userFeedback: String?,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<DebuggingArchive>("debuggingArchive", generatedSerializer())

        // Eventually, all platforms should just use the JSON FFI command to create debug archives, but for now,
        // adapting the command here is the least invasive change.
        // TODO: https://linear.app/soveng/issue/OBS-3095/cross-platform-debug-archive-story
        override fun run(args: Args): CompletableFuture<String> {
            args.osStatusManager.update { this.debugBundleStatus.inProgress = true }
            return JsonFfiCommand(jsonConfig.encodeToString(ManagerCmd.CreateDebugArchive(userFeedback)))
                .run(args)
                .whenComplete { result: String?, _ ->
                    args.osStatusManager.update {
                        this.debugBundleStatus.inProgress = false
                        if (result != null) {
                            this.debugBundleStatus.latestPath = jsonConfig.decodeFromString(result)
                        }
                    }
                }
        }
    }

    @KeepGeneratedSerializer
    @Serializable(with = EmailDebugArchive.Serializer::class)
    class EmailDebugArchive(
        val path: String,
        val subject: String,
        val body: String,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<EmailDebugArchive>("emailDebugArchive", generatedSerializer())

        override fun run(args: Args) =
            completedVoid().also { shareDebugArchive(args.context, path, true, subject, body) }
    }

    @KeepGeneratedSerializer
    @Serializable(with = GetOsStatus.Serializer::class)
    class GetOsStatus(val knownVersion: String? = null) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<GetOsStatus>("getOsStatus", generatedSerializer())

        override fun run(args: Args): CompletableFuture<String> =
            args.osStatusManager.getStatus(this.knownVersion).thenApply { jsonConfig.encodeToString(it) }
    }

    @KeepGeneratedSerializer
    @Serializable(with = JsonFfiCommand.Serializer::class)
    class JsonFfiCommand(
        val cmd: String,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<JsonFfiCommand>("jsonFfiCmd", generatedSerializer())

        override fun run(args: Args) = WebCmdBridge.Receiver.register { id -> args.binder.jsonFfi(id, this.cmd) }
    }

    @KeepGeneratedSerializer
    @Serializable(with = PurchaseSubscription.Serializer::class)
    class PurchaseSubscription : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<PurchaseSubscription>(
                "purchaseSubscription",
                generatedSerializer(),
            )

        override fun run(args: Args) = args.mainActivity.billingFacade.launchFlow(args.mainActivity)
    }

    @KeepGeneratedSerializer
    @Serializable(with = RevealItemInDir.Serializer::class)
    class RevealItemInDir(
        val path: String,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<RevealItemInDir>(
                "revealItemInDir",
                generatedSerializer(),
            )

        override fun run(args: Args) = completedUnsupported()
    }

    @KeepGeneratedSerializer
    @Serializable(with = SetColorScheme.Serializer::class)
    class SetColorScheme(
        val value: Preferences.ColorScheme,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<SetColorScheme>(
                "setColorScheme",
                generatedSerializer(),
            )

        override fun run(args: Args) = completedVoid().also { Preferences(args.context).colorScheme = this.value }
    }

    @KeepGeneratedSerializer
    @Serializable(with = SetFeatureFlag.Serializer::class)
    class SetFeatureFlag(
        val flag: String,
        val active: Boolean,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<SetFeatureFlag>(
                "setFeatureFlag",
                generatedSerializer(),
            )

        override fun run(args: Args) = completedUnsupported()
    }

    @KeepGeneratedSerializer
    @Serializable(with = ShareDebugArchive.Serializer::class)
    class ShareDebugArchive(
        val path: String,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<ShareDebugArchive>(
                "shareDebugArchive",
                generatedSerializer(),
            )

        override fun run(args: Args) = completedVoid().also { shareDebugArchive(args.context, path, false) }
    }

    @KeepGeneratedSerializer
    @Serializable(with = StartTunnel.Serializer::class)
    class StartTunnel(
        val tunnelArgs: String? = null,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<StartTunnel>(
                "startTunnel",
                generatedSerializer(),
            )

        override fun run(args: Args) =
            args.mainActivity.lifecycleScope.future {
                args.mainActivity.vpnPermissionRequestManager
                    .requestVpnStart()
                    .getOrThrow()
                    .let { jsonConfig.encodeToString(it) }
                    .also { args.binder.startTunnel(this@StartTunnel.tunnelArgs) }
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

        override fun run(args: Args) = completedVoid().also { args.binder.stopTunnel() }
    }
}

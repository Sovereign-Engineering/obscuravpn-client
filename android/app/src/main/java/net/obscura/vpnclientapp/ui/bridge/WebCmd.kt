package net.obscura.vpnclientapp.ui.bridge

import android.content.Context
import kotlinx.serialization.KeepGeneratedSerializer
import kotlinx.serialization.Serializable
import net.obscura.lib.util.ExternallyTaggedEnumSerializer
import net.obscura.lib.util.ExternallyTaggedEnumVariantSerializer
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.ErrorCodeException
import net.obscura.vpnclientapp.client.ManagerCmd
import net.obscura.vpnclientapp.client.ManagerCmdOk
import net.obscura.vpnclientapp.client.jsonConfig
import net.obscura.vpnclientapp.preferences.Preferences
import net.obscura.vpnclientapp.services.IObscuraVpnService
import net.obscura.vpnclientapp.ui.JsonFfiBroadcastReceiver
import net.obscura.vpnclientapp.ui.ObscuraUI
import net.obscura.vpnclientapp.ui.OsStatus
import net.obscura.vpnclientapp.ui.OsStatusManager
import net.obscura.vpnclientapp.ui.uploadPurchaseToken

private val jsonUnit = jsonConfig.encodeToString(Unit)
private val log = Logger(WebCmd::class)

data class WebCmdArgs(
    val context: Context,
    val binder: IObscuraVpnService,
    val mainActivity: MainActivity,
    val osStatusManager: OsStatusManager,
    val ui: ObscuraUI,
)

@Serializable(with = WebCmd.Serializer::class)
internal sealed interface WebCmd {
    object Serializer :
        ExternallyTaggedEnumSerializer<WebCmd>(
            WebCmd::class,
            listOf(
                DebugBundle.Serializer,
                EmailDebugBundle.Serializer,
                GetOsStatus.Serializer,
                JsonFfiCmd.Serializer,
                PurchaseSubscription.Serializer,
                RevealItemInDir.Serializer,
                SetColorScheme.Serializer,
                SetFeatureFlag.Serializer,
                SetNavigationView.Serializer,
                ShareDebugBundle.Serializer,
                StartTunnel.Serializer,
                StopTunnel.Serializer,
            ),
        )

    suspend fun run(args: WebCmdArgs): String

    @KeepGeneratedSerializer
    @Serializable(with = DebugBundle.Serializer::class)
    data class DebugBundle(
        val userFeedback: String?,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<DebugBundle>("debugBundle", generatedSerializer())

        // Eventually, all platforms should just use the JSON FFI command to create debug bundles, but for now,
        // adapting the command here is the least invasive change.
        // TODO: https://linear.app/soveng/issue/OBS-3095/cross-platform-debug-archive-story
        override suspend fun run(args: WebCmdArgs) =
            jsonUnit.also {
                args.osStatusManager.update { this.debugBundleStatus.inProgress = true }
                val path = runCatching {
                    JsonFfiCmd(jsonConfig.encodeToString(ManagerCmd.CreateDebugBundle(userFeedback))).run(args).let {
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
    @Serializable(with = EmailDebugBundle.Serializer::class)
    data class EmailDebugBundle(
        val path: String,
        val subject: String,
        val body: String,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<EmailDebugBundle>("emailDebugBundle", generatedSerializer())

        override suspend fun run(args: WebCmdArgs) =
            jsonUnit.also { shareDebugBundle(args.context, path, true, subject, body) }
    }

    @KeepGeneratedSerializer
    @Serializable(with = GetOsStatus.Serializer::class)
    data class GetOsStatus(val knownVersion: String?) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<GetOsStatus>("getOsStatus", generatedSerializer())

        override suspend fun run(args: WebCmdArgs) = args.osStatusManager.waitForUpdate(knownVersion).await()
    }

    @KeepGeneratedSerializer
    @Serializable(with = JsonFfiCmd.Serializer::class)
    data class JsonFfiCmd(
        val cmd: String,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<JsonFfiCmd>("jsonFfiCmd", generatedSerializer())

        override suspend fun run(args: WebCmdArgs) =
            JsonFfiBroadcastReceiver.waitForResponse(args.binder, this.cmd).await()
    }

    @KeepGeneratedSerializer
    @Serializable(with = PurchaseSubscription.Serializer::class)
    data class PurchaseSubscription(val promoCode: String?) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<PurchaseSubscription>(
                "purchaseSubscription",
                generatedSerializer(),
            )

        override suspend fun run(args: WebCmdArgs): String {
            val billingDetails =
                JsonFfiCmd(
                        jsonConfig.encodeToString(ManagerCmd.ApiGoogleBillingDetails(this.promoCode)),
                    )
                    .run(args)
                    .let { jsonConfig.decodeFromString<ManagerCmdOk.ApiGoogleBillingDetails>(it) }
            log.info("billing details: $billingDetails")
            val purchaseTokens = args.mainActivity.billingFacade.launchFlow(args.mainActivity, billingDetails)
            val didPurchase = purchaseTokens != null
            if (didPurchase) {
                for (purchaseToken in purchaseTokens) {
                    uploadPurchaseToken(args.binder, purchaseToken, promoCode)
                }
            }
            return jsonConfig.encodeToString(didPurchase)
        }
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

        override suspend fun run(args: WebCmdArgs) = throw ErrorCodeException.unsupportedOnOS()
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

        override suspend fun run(args: WebCmdArgs) =
            jsonUnit.also { Preferences(args.context).colorScheme = this.value }
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

        override suspend fun run(args: WebCmdArgs) = throw ErrorCodeException.unsupportedOnOS()
    }

    @KeepGeneratedSerializer
    @Serializable(with = SetNavigationView.Serializer::class)
    data class SetNavigationView(
        val view: OsStatus.NavigationView,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<SetNavigationView>(
                "setNavigationView",
                generatedSerializer(),
            )

        override suspend fun run(args: WebCmdArgs) = jsonUnit.also { args.ui.setNavigationView(this.view) }
    }

    @KeepGeneratedSerializer
    @Serializable(with = ShareDebugBundle.Serializer::class)
    data class ShareDebugBundle(
        val path: String,
    ) : WebCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<ShareDebugBundle>(
                "shareDebugBundle",
                generatedSerializer(),
            )

        override suspend fun run(args: WebCmdArgs) = jsonUnit.also { shareDebugBundle(args.context, path, false) }
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

        override suspend fun run(args: WebCmdArgs) =
            jsonUnit.also {
                args.mainActivity.vpnPermissionRequestManager.requestVpnStart(this@StartTunnel.tunnelArgs).getOrThrow()
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

        override suspend fun run(args: WebCmdArgs) = jsonUnit.also { args.binder.stopTunnel() }
    }
}

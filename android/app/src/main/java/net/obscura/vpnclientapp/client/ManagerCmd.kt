package net.obscura.vpnclientapp.client

import kotlinx.serialization.KeepGeneratedSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject
import net.obscura.lib.util.ExternallyTaggedEnumVariantSerializer

sealed interface ManagerCmd {
    @KeepGeneratedSerializer
    @Serializable(with = ApiGoogleAssociateAccount.Serializer::class)
    data class ApiGoogleAssociateAccount(
        val purchaseToken: String,
        val promoCode: String?,
    ) : ManagerCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<ApiGoogleAssociateAccount>(
                "apiGoogleAssociateAccount",
                generatedSerializer(),
            )
    }

    @KeepGeneratedSerializer
    @Serializable(with = ApiGoogleBillingDetails.Serializer::class)
    data class ApiGoogleBillingDetails(
        val promoCode: String?,
    ) : ManagerCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<ApiGoogleBillingDetails>(
                "apiGoogleBillingDetails",
                generatedSerializer(),
            )
    }

    @KeepGeneratedSerializer
    @Serializable(with = CreateDebugBundle.Serializer::class)
    data class CreateDebugBundle(
        val userFeedback: String?,
        val bundleInfo: BundleInfo,
    ) : ManagerCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<CreateDebugBundle>("createDebugBundle", generatedSerializer())

        @Serializable
        data class BundleInfo(
            val androidSdk: Int?,
            val appVersion: String,
            val bootTimestamp: String?,
            val brand: String?,
            val lowPowerMode: Boolean?,
            val memoryAvailGib: Double?,
            val memoryTotalGib: Double?,
            val model: String?,
            val osVersion: String?,
            val processId: Int?,
            val processName: String?,
            val processorCountActive: Int?,
            val processorName: String?,
            val thermalState: String?,
            val uptimeHours: Double?,
        )
    }

    @KeepGeneratedSerializer
    @Serializable(with = GetStatus.Serializer::class)
    data class GetStatus(val knownVersion: String?) : ManagerCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<GetStatus>("getStatus", generatedSerializer())
    }

    @KeepGeneratedSerializer
    @Serializable(with = SetTunnelArgs.Serializer::class)
    data class SetTunnelArgs(
        val args: Map<String, JsonObject>? = null,
        val active: Boolean? = null,
    ) : ManagerCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<SetTunnelArgs>("setTunnelArgs", generatedSerializer())
    }
}

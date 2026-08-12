package net.obscura.vpnclientapp.client

import kotlinx.serialization.KeepGeneratedSerializer
import kotlinx.serialization.SerialName
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
            @SerialName("AndroidSDK") val androidSdk: Int?,
            @SerialName("AppVersion") val appVersion: String,
            @SerialName("BootTimestamp") val bootTimestamp: String?,
            @SerialName("Brand") val brand: String?,
            @SerialName("LowPowerMode") val lowPowerMode: Boolean?,
            @SerialName("Model") val model: String?,
            @SerialName("OSVersionString") val osVersionString: String?,
            @SerialName("PID") val pid: Int?,
            @SerialName("ProcessName") val processName: String?,
            @SerialName("ProcessorCountActive") val processorCountActive: Int?,
            @SerialName("ProcessorName") val processorName: String?,
            @SerialName("RAMAvailableGiB") val ramAvailableGiB: Double?,
            @SerialName("RAMLogicalGiB") val ramLogicalGiB: Double?,
            @SerialName("ThermalState") val thermalState: String?,
            @SerialName("UptimeHours") val uptimeHours: Double?,
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

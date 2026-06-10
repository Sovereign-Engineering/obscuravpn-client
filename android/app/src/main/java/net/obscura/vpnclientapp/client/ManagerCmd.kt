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
    ) : ManagerCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<CreateDebugBundle>("createDebugBundle", generatedSerializer())
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

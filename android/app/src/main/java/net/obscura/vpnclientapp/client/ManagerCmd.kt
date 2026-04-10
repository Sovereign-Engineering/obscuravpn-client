package net.obscura.vpnclientapp.client

import kotlinx.serialization.KeepGeneratedSerializer
import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject
import net.obscura.lib.util.ExternallyTaggedEnumVariantSerializer

sealed interface ManagerCmd {
    @KeepGeneratedSerializer
    @Serializable(with = CreateDebugArchive.Serializer::class)
    data class CreateDebugArchive(
        val userFeedback: String?,
    ) : ManagerCmd {
        internal object Serializer :
            ExternallyTaggedEnumVariantSerializer<CreateDebugArchive>("createDebugArchive", generatedSerializer())
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

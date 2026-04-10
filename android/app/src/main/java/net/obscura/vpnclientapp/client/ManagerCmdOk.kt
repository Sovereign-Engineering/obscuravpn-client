package net.obscura.vpnclientapp.client

import kotlinx.serialization.KeepGeneratedSerializer
import kotlinx.serialization.Serializable
import net.obscura.lib.util.ExternallyTaggedEnumSerializer
import net.obscura.lib.util.ExternallyTaggedEnumVariantSerializer

sealed interface ManagerCmdOk {
    @Serializable
    data class GetStatus(
        val accountId: String?,
        val autoConnect: Boolean,
        val inNewAccountFlow: Boolean,
        val version: String,
        val vpnStatus: VpnStatus,
    ) : ManagerCmdOk {
        @Serializable(with = VpnStatus.Serializer::class)
        sealed interface VpnStatus {
            object Serializer :
                ExternallyTaggedEnumSerializer<VpnStatus>(
                    VpnStatus::class,
                    listOf(
                        Connected.Serializer,
                        Connecting.Serializer,
                        Disconnected.Serializer,
                    ),
                )

            @KeepGeneratedSerializer
            @Serializable(with = Connected.Serializer::class)
            class Connected : VpnStatus {
                internal object Serializer :
                    ExternallyTaggedEnumVariantSerializer<Connected>("connected", generatedSerializer())
            }

            @KeepGeneratedSerializer
            @Serializable(with = Connecting.Serializer::class)
            class Connecting : VpnStatus {
                internal object Serializer :
                    ExternallyTaggedEnumVariantSerializer<Connecting>("connecting", generatedSerializer())
            }

            @KeepGeneratedSerializer
            @Serializable(with = Disconnected.Serializer::class)
            class Disconnected : VpnStatus {
                internal object Serializer :
                    ExternallyTaggedEnumVariantSerializer<Disconnected>("disconnected", generatedSerializer())
            }
        }
    }
}

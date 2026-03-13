package net.obscura.vpnclientapp.client.commands

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject

@Serializable
data class GetStatus(
    val getStatus: Request,
) {
    @Serializable
    data class Request(
        val knownVersion: String? = null,
    )

    @Serializable
    data class Response(
        val version: String? = null,
        val accountId: String? = null,
        val inNewAccountFlow: Boolean,
        val vpnStatus: VpnStatus,
    ) {
        @Serializable
        data class VpnStatus(
            val disconnected: JsonObject? = null,
            val connected: JsonObject? = null,
            val connecting: JsonObject? = null,
        )
    }
}

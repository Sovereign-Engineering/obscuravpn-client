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
      val vpnStatus: VpnStatus? = null,
  ) {
    @Serializable
    data class VpnStatus(
        val disconnected: JsonObject? = null,
        val connected: Connected? = null,
        val connecting: JsonObject? = null,
    ) {
      @Serializable
      data class Connected(
          val networkConfig: NetworkConfig,
      ) {
        @Serializable
        data class NetworkConfig(
            val dns: ArrayList<String?>? = null,
            val ipv4: String? = null,
            val ipv6: String? = null,
            val mtu: Int? = null,
        )
      }
    }
  }
}

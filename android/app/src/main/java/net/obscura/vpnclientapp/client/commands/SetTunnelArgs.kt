package net.obscura.vpnclientapp.client.commands

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject

@Serializable
data class SetTunnelArgs(
    val setTunnelArgs: Request,
) {
  @Serializable
  data class Request(
      val args: Map<String, JsonObject>? = null,
      val allowActivation: Boolean? = null,
  )
}

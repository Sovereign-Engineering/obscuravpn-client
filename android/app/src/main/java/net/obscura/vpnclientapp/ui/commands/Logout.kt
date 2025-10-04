package net.obscura.vpnclientapp.ui.commands

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.JsonObject

@Serializable()
data class Logout(val _x: JsonObject?) {
    fun run(): Any {
        TODO()
    }
}

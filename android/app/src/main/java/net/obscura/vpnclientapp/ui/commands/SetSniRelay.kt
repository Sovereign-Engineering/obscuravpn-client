package net.obscura.vpnclientapp.ui.commands

import kotlinx.serialization.Serializable

@Serializable
data class SetSniRelay(val host: String?) {
    fun run(): Any {
        TODO()
    }
}

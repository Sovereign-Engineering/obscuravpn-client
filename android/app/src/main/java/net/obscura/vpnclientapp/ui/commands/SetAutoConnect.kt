package net.obscura.vpnclientapp.ui.commands

import kotlinx.serialization.Serializable

@Serializable
data class SetAutoConnect(val enable: Boolean?) {
    fun run(): Any {
        TODO()
    }
}

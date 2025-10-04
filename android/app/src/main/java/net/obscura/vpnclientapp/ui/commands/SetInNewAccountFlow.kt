package net.obscura.vpnclientapp.ui.commands

import kotlinx.serialization.Serializable

@Serializable
data class SetInNewAccountFlow(val value: Boolean?) {
    fun run(): Any {
        TODO()
    }
}

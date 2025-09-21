package net.obscura.vpnclientapp.ui.commands

import kotlinx.serialization.Serializable

@Serializable
data class SetApiHostAlternate(val host: String?) {
    fun run(): Any {
        TODO()
    }
}

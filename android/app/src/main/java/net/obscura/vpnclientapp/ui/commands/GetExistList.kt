package net.obscura.vpnclientapp.ui.commands

import kotlinx.serialization.Serializable

@Serializable
data class GetExistList(val version: String?) {
    fun run(): Any {
        TODO()
    }
}

package net.obscura.vpnclientapp.ui.commands

import kotlinx.serialization.Serializable

@Serializable
data class SetApiUrl(
    val url: String?
) {
    fun run(): Any {
        TODO()
    }
}

package net.obscura.vpnclientapp.ui.commands

import kotlinx.serialization.Serializable

@Serializable
data class Login(
    val accountId: String?,
    val validate: Boolean?,
) {
    fun run(): Any {
        TODO()
    }
}

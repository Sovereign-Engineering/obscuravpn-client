package net.obscura.vpnclientapp.ui.commands

import kotlinx.serialization.Serializable

@Serializable
data class StartTunnel(
    val tunnelArgs: String? = null,
)

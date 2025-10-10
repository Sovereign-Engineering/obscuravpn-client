package net.obscura.vpnclientapp.ui.commands

import kotlinx.serialization.Serializable

@Serializable
data class SetFeatureFlag(
    val flag: String?,
    val active: Boolean?
) {
    fun run(): Any {
        TODO()
    }
}

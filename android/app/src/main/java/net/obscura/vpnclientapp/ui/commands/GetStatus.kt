package net.obscura.vpnclientapp.ui.commands

import androidx.annotation.AnyRes
import kotlinx.serialization.Serializable

@Serializable
data class GetStatus(
    val knownVersion: String?
) {
    fun run(): Any {
        TODO()
    }
}

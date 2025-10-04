package net.obscura.vpnclientapp.ui.commands

import kotlinx.serialization.Serializable

@Serializable
data class RefreshExitList(val freshness: Long?) {
    fun run(): Any {
        TODO()
    }
}

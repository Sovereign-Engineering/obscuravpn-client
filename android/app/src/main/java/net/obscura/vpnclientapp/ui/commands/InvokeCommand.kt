package net.obscura.vpnclientapp.ui.commands

import kotlinx.serialization.Serializable

@Serializable
data class InvokeCommand(
    val jsonFfiCommand: JsonFfiCommand?
) {
    fun run(): Any {
        return when {
            jsonFfiCommand != null -> {
                jsonFfiCommand.run()
            }

            else -> {
                throw NotImplementedError("InvokeCommand not implemented")
            }
        }
    }
}

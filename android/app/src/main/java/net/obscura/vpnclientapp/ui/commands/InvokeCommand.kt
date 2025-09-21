package net.obscura.vpnclientapp.ui.commands

import kotlinx.serialization.Serializable

@Serializable
data class InvokeCommand(
    val getOsStatus: GetOsStatus? = null,
    val jsonFfiCommand: JsonFfiCommand? = null,
) {
    fun run(): Any {
        return when {
            getOsStatus != null -> getOsStatus.run()

            jsonFfiCommand != null -> jsonFfiCommand.run()

            else ->
                throw NotImplementedError("InvokeCommand not implemented")
        }
    }
}

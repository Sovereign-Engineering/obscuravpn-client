package net.obscura.vpnclientapp.ui.commands

import kotlinx.serialization.Serializable
import kotlinx.serialization.json.Json

@Serializable
data class InvokeCommand(
    val getOsStatus: GetOsStatus? = null,
    val jsonFfiCommand: JsonFfiCommand? = null,
) {
    fun run(json: Json): String {
        return when {
            getOsStatus != null -> json.encodeToString(getOsStatus.run())

            jsonFfiCommand != null -> json.encodeToString(jsonFfiCommand.run())

            else ->
                throw NotImplementedError("InvokeCommand not implemented")
        }
    }
}

package net.obscura.vpnclientapp.ui.commands

import kotlinx.serialization.Serializable

@Serializable
data class SetPinnedExits(val exits: Array<PinnedLocation>?) {

    @Serializable
    data class PinnedLocation(
        val country_code: String,
        val city_code: String,
        val pinned_at: Long,
    )

    fun run(): Any {
        TODO()
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (javaClass != other?.javaClass) return false

        other as SetPinnedExits

        if (!exits.contentEquals(other.exits)) return false

        return true
    }

    override fun hashCode(): Int {
        return exits?.contentHashCode() ?: 0
    }
}

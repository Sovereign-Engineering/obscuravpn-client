package net.obscura.vpnclientapp

import android.content.Context
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.errorCodeUnsupportedOnOS

class BillingFacade(@Suppress("UNUSED_PARAMETER") context: Context) {
    @Suppress("RedundantSuspendModifier")
    suspend fun launchFlow(@Suppress("UNUSED_PARAMETER") mainActivity: MainActivity): Boolean =
        throw errorCodeUnsupportedOnOS()

    fun destroy() = Unit
}

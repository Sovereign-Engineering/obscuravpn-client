package net.obscura.vpnclientapp

import android.content.Context
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.errorCodeUnsupportedOnOS
import net.obscura.vpnclientapp.services.IObscuraVpnService

class BillingFacade(@Suppress("UNUSED_PARAMETER") context: Context) {
    @Suppress("RedundantSuspendModifier")
    suspend fun associateKnownPurchaseTokens(@Suppress("UNUSED_PARAMETER") binder: IObscuraVpnService) = Unit

    @Suppress("RedundantSuspendModifier")
    suspend fun launchFlow(
        @Suppress("UNUSED_PARAMETER") binder: IObscuraVpnService,
        @Suppress("UNUSED_PARAMETER") mainActivity: MainActivity,
    ): Boolean = throw errorCodeUnsupportedOnOS()

    fun destroy() = Unit
}

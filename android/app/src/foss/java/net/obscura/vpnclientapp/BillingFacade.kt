package net.obscura.vpnclientapp

import android.content.Context
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.ReceiveChannel
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.errorCodeUnsupportedOnOS

class BillingFacade(@Suppress("UNUSED_PARAMETER") context: Context) {
    val purchaseTokensRx: ReceiveChannel<String> = Channel()

    init {
        this.purchaseTokensRx.cancel(CancellationException("Play Billing not in use"))
    }

    @Suppress("RedundantSuspendModifier") suspend fun refreshPurchaseTokens() = Unit

    @Suppress("RedundantSuspendModifier")
    suspend fun launchFlow(
        @Suppress("UNUSED_PARAMETER") mainActivity: MainActivity,
    ): Boolean = throw errorCodeUnsupportedOnOS()

    fun destroy() = Unit
}

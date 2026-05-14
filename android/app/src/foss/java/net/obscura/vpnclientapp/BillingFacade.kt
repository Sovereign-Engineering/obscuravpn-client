package net.obscura.vpnclientapp

import android.content.Context
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.ManagerCmdOk
import net.obscura.vpnclientapp.client.errorCodeUnsupportedOnOS

class BillingFacade(@Suppress("UNUSED_PARAMETER") context: Context) {
    @Suppress("FunctionOnlyReturningConstant", "RedundantSuspendModifier")
    suspend fun fetchPurchaseTokens(): List<String>? = null

    @Suppress("RedundantNullableReturnType", "RedundantSuspendModifier")
    suspend fun launchFlow(
        @Suppress("UNUSED_PARAMETER") mainActivity: MainActivity,
        @Suppress("UNUSED_PARAMETER") billingDetails: ManagerCmdOk.ApiGoogleBillingDetails,
    ): List<String>? = throw errorCodeUnsupportedOnOS()

    fun destroy() = Unit
}

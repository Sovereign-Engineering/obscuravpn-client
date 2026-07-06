package net.obscura.vpnclientapp.ui

import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.ErrorCodeException
import net.obscura.vpnclientapp.client.ManagerCmdOk

interface BillingFacade {
    fun isPlayBilling() = false

    suspend fun fetchPurchaseTokens(): List<String>? = null

    suspend fun launchFlow(
        mainActivity: MainActivity,
        billingDetails: ManagerCmdOk.ApiGoogleBillingDetails,
    ): List<String>? = throw ErrorCodeException.unsupportedOnOS()

    fun destroy() = Unit
}

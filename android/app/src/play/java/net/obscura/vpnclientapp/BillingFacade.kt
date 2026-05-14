package net.obscura.vpnclientapp

import android.content.Context
import net.obscura.lib.billing.BillingManager
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.ManagerCmdOk
import net.obscura.vpnclientapp.client.errorCodePurchaseFailed
import net.obscura.vpnclientapp.client.errorCodePurchaseFailedAlreadyOwned

class BillingFacade(context: Context) {
    private val billingManager = BillingManager(context)

    suspend fun fetchPurchaseTokens() = this.billingManager.fetchPurchaseTokens()

    suspend fun launchFlow(mainActivity: MainActivity, billingDetails: ManagerCmdOk.ApiGoogleBillingDetails) =
        when (
            val result =
                this.billingManager.launchFlow(
                    mainActivity,
                    billingDetails.productId,
                    billingDetails.basePlanId,
                    billingDetails.offerId,
                )
        ) {
            is BillingManager.PurchaseResult.Completed -> result.purchaseTokens
            BillingManager.PurchaseResult.Canceled -> null
            BillingManager.PurchaseResult.AlreadyOwned -> throw errorCodePurchaseFailedAlreadyOwned()
            BillingManager.PurchaseResult.Failed -> throw errorCodePurchaseFailed()
        }

    fun destroy() = this.billingManager.destroy()
}

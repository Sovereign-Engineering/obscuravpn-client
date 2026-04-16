package net.obscura.vpnclientapp

import android.content.Context
import net.obscura.lib.billing.BillingManager
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.errorCodePurchaseFailed
import net.obscura.vpnclientapp.client.errorCodePurchaseFailedAlreadyOwned

class BillingFacade(context: Context) {
    private val billingManager = BillingManager(context)

    suspend fun launchFlow(mainActivity: MainActivity) =
        when (this@BillingFacade.billingManager.launchFlow(mainActivity)) {
            BillingManager.PurchaseResult.Completed -> true
            BillingManager.PurchaseResult.Canceled -> false
            BillingManager.PurchaseResult.AlreadyOwned -> throw errorCodePurchaseFailedAlreadyOwned()
            BillingManager.PurchaseResult.Failed -> throw errorCodePurchaseFailed()
        }

    fun destroy() = this.billingManager.destroy()
}

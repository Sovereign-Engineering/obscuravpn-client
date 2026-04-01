package net.obscura.vpnclientapp

import android.content.Context
import androidx.lifecycle.lifecycleScope
import java.util.concurrent.CompletableFuture
import kotlinx.coroutines.future.future
import kotlinx.serialization.json.Json
import net.obscura.lib.billing.BillingManager
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.errorCodePurchaseFailed
import net.obscura.vpnclientapp.client.errorCodePurchaseFailedAlreadyOwned

class BillingFacade(context: Context) {
    private val billingManager = BillingManager(context)

    fun launchFlow(mainActivity: MainActivity): CompletableFuture<String> =
        mainActivity.lifecycleScope.future {
            when (this@BillingFacade.billingManager.launchFlow(mainActivity)) {
                BillingManager.PurchaseResult.Completed -> true
                BillingManager.PurchaseResult.Canceled -> false
                BillingManager.PurchaseResult.AlreadyOwned -> throw errorCodePurchaseFailedAlreadyOwned()
                BillingManager.PurchaseResult.Failed -> throw errorCodePurchaseFailed()
            }.let { Json.encodeToString(it) }
        }

    fun destroy() = this.billingManager.destroy()
}

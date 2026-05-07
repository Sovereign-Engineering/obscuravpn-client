package net.obscura.vpnclientapp

import android.content.Context
import net.obscura.lib.billing.BillingManager
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.ManagerCmd
import net.obscura.vpnclientapp.client.errorCodePurchaseFailed
import net.obscura.vpnclientapp.client.errorCodePurchaseFailedAlreadyOwned
import net.obscura.vpnclientapp.client.jsonConfig
import net.obscura.vpnclientapp.services.IObscuraVpnService
import net.obscura.vpnclientapp.ui.JsonFfiBroadcastReceiver

private val log = Logger(BillingFacade::class)

class BillingFacade(context: Context) {
    private suspend fun associateAccount(
        binder: IObscuraVpnService,
        purchaseTokens: List<String>,
    ) {
        log.info("associating purchase tokens with account: $purchaseTokens")
        for (purchaseToken in purchaseTokens) {
            JsonFfiBroadcastReceiver.waitForResponse(
                    binder,
                    jsonConfig.encodeToString(ManagerCmd.ApiGoogleAssociateAccount(purchaseToken)),
                )
                .await()
        }
    }

    private val billingManager = BillingManager(context)

    suspend fun associateKnownPurchaseTokens(binder: IObscuraVpnService) =
        this.associateAccount(binder, this.billingManager.knownPurchaseTokens())

    suspend fun launchFlow(binder: IObscuraVpnService, mainActivity: MainActivity) =
        when (val result = this.billingManager.launchFlow(mainActivity)) {
            is BillingManager.PurchaseResult.Completed -> {
                this.associateAccount(binder, result.purchaseTokens)
                true
            }
            BillingManager.PurchaseResult.Canceled -> false
            BillingManager.PurchaseResult.AlreadyOwned -> throw errorCodePurchaseFailedAlreadyOwned()
            BillingManager.PurchaseResult.Failed -> throw errorCodePurchaseFailed()
        }

    fun destroy() = this.billingManager.destroy()
}

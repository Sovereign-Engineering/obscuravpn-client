package net.obscura.vpnclientapp

import android.content.Context
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import net.obscura.lib.billing.BillingManager
import net.obscura.lib.billing.PlayServicesManager
import net.obscura.lib.billing.PlayServicesManager.AvailabilityResult
import net.obscura.lib.billing.PlayServicesManager.inactiveReason
import net.obscura.lib.billing.PlayServicesManager.isMissing
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.ErrorCodeException
import net.obscura.vpnclientapp.client.ManagerCmdOk
import net.obscura.vpnclientapp.ui.BillingFacade

class BillingImpl(private val context: Context) : BillingFacade {
    private val lock = Mutex()
    private var availabilityResult = PlayServicesManager.checkAvailability(context)

    override fun isPlayBilling() = !this.availabilityResult.isMissing()

    private val billingManager =
        if (this.isPlayBilling()) {
            BillingManager(context)
        } else {
            null
        }

    override suspend fun fetchPurchaseTokens() = this.billingManager?.fetchPurchaseTokens()

    override suspend fun launchFlow(
        mainActivity: MainActivity,
        billingDetails: ManagerCmdOk.ApiGoogleBillingDetails,
    ) =
        this.lock.withLock {
            this.availabilityResult = PlayServicesManager.checkAvailability(this.context)
            this.availabilityResult.inactiveReason()?.let {
                if (PlayServicesManager.makeAvailable(mainActivity)) {
                    this.availabilityResult = AvailabilityResult.Present(null)
                }
            }
            when (
                val result =
                    this.billingManager?.launchFlow(
                        mainActivity,
                        billingDetails.productId,
                        billingDetails.basePlanId,
                        billingDetails.offerId,
                    )
            ) {
                is BillingManager.PurchaseResult.Completed -> result.purchaseTokens
                BillingManager.PurchaseResult.Canceled -> null
                BillingManager.PurchaseResult.AlreadyOwned -> throw ErrorCodeException.purchaseFailedAlreadyOwned()
                BillingManager.PurchaseResult.Failed -> {
                    throw when (this.availabilityResult.inactiveReason()) {
                        AvailabilityResult.Present.InactiveReason.Disabled -> ErrorCodeException.playServicesDisabled()
                        AvailabilityResult.Present.InactiveReason.UpdateRequired ->
                            ErrorCodeException.playServicesUpdateRequired()
                        AvailabilityResult.Present.InactiveReason.Updating -> ErrorCodeException.playServicesUpdating()
                        AvailabilityResult.Present.InactiveReason.Unknown -> ErrorCodeException.other()
                        null ->
                            if (this.availabilityResult.isMissing()) ErrorCodeException.playServicesMissing()
                            else ErrorCodeException.purchaseFailed()
                    }
                }
                null -> throw ErrorCodeException.playServicesMissing()
            }
        }

    override fun destroy() {
        this.billingManager?.destroy()
    }
}

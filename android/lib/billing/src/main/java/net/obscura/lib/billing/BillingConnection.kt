package net.obscura.lib.billing

import android.content.Context
import com.android.billingclient.api.BillingClient
import com.android.billingclient.api.BillingClientStateListener
import com.android.billingclient.api.BillingResult
import com.android.billingclient.api.PendingPurchasesParams
import com.android.billingclient.api.Purchase
import com.android.billingclient.api.PurchasesUpdatedListener
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.asSharedFlow
import net.obscura.lib.util.Logger

private val log = Logger(BillingConnection::class)

internal class BillingConnection<T>(
    context: Context,
    purchasesUpdatedListenerCallback: (BillingResult, List<Purchase>?) -> T,
) {
    private val purchaseUpdatesTx = MutableSharedFlow<T>(extraBufferCapacity = 1)
    val purchaseUpdatesRx = this.purchaseUpdatesTx.asSharedFlow()

    private val purchasesUpdatedListener = PurchasesUpdatedListener { result, purchases ->
        log.info("purchases updated: $result $purchases")
        val wasEmitted = purchaseUpdatesTx.tryEmit(purchasesUpdatedListenerCallback(result, purchases))
        if (!wasEmitted) {
            log.warn("multiple purchase updates while collecting")
        }
    }

    val client =
        BillingClient.newBuilder(context)
            .setListener(this.purchasesUpdatedListener)
            .enableAutoServiceReconnection()
            .enablePendingPurchases(
                PendingPurchasesParams.newBuilder()
                    .enableOneTimeProducts() // This is mandatory
                    .build()
            )
            .build()

    init {
        log.debug("starting billing connection")
        // Calling this doesn't appear to be necessary when using `enableAutoServiceReconnection`, but the callbacks can
        // still be useful for:
        // 1. Querying purchases/etc. at the earliest possible time
        // 2. Logging
        client.startConnection(
            object : BillingClientStateListener {
                override fun onBillingSetupFinished(result: BillingResult) {
                    if (result.responseCode == BillingClient.BillingResponseCode.OK) {
                        log.info("billing setup succeeded: $result")
                    } else {
                        log.error("billing setup failed: $result")
                    }
                }

                override fun onBillingServiceDisconnected() {
                    log.info("billing client disconnected")
                }
            }
        )
    }

    fun destroy() {
        log.debug("destroying billing connection")
        this.client.endConnection()
    }
}

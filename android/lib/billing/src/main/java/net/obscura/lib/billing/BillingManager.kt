package net.obscura.lib.billing

import android.app.Activity
import android.content.Context
import com.android.billingclient.api.BillingClient
import com.android.billingclient.api.BillingFlowParams
import com.android.billingclient.api.ProductDetails
import com.android.billingclient.api.QueryProductDetailsParams
import com.android.billingclient.api.queryProductDetails
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.firstOrNull
import kotlinx.coroutines.flow.onSubscription
import kotlinx.coroutines.withContext
import net.obscura.lib.util.Logger

private val log = Logger(BillingManager::class)

private const val PRODUCT_ID = "vpn_subscription_v1"
private const val BASE_PLAN_ID = "monthly-autorenewing"

class BillingManager(context: Context) {
    sealed interface PurchaseResult {
        object Completed : PurchaseResult

        object Canceled : PurchaseResult

        object AlreadyOwned : PurchaseResult

        object Failed : PurchaseResult
    }

    private val connection =
        BillingConnection(context) { result, _ ->
            when (result.responseCode) {
                BillingClient.BillingResponseCode.OK -> PurchaseResult.Completed
                BillingClient.BillingResponseCode.USER_CANCELED -> PurchaseResult.Canceled
                BillingClient.BillingResponseCode.ITEM_ALREADY_OWNED -> PurchaseResult.AlreadyOwned
                else -> {
                    log.error("purchase failed: $result")
                    PurchaseResult.Failed
                }
            }
        }

    private data class SubscriptionDetails(
        val productDetails: ProductDetails,
        val offerDetails: ProductDetails.SubscriptionOfferDetails,
    )

    private suspend fun querySubscriptionDetails(client: BillingClient): SubscriptionDetails? {
        val result =
            withContext(Dispatchers.IO) {
                client.queryProductDetails(
                    QueryProductDetailsParams.newBuilder()
                        .setProductList(
                            listOf(
                                QueryProductDetailsParams.Product.newBuilder()
                                    .setProductId(PRODUCT_ID)
                                    .setProductType(BillingClient.ProductType.SUBS)
                                    .build()
                            )
                        )
                        .build()
                )
            }
        return when (result.billingResult.responseCode) {
            BillingClient.BillingResponseCode.OK -> {
                val productDetails = result.productDetailsList?.find { it.productId == PRODUCT_ID }
                val offerDetails = productDetails?.subscriptionOfferDetails?.find { it.basePlanId == BASE_PLAN_ID }
                if (offerDetails != null) {
                    log.info("subscription details: $productDetails $offerDetails")
                    SubscriptionDetails(productDetails, offerDetails)
                } else {
                    log.error(
                        "subscription details for product $PRODUCT_ID and base plan $BASE_PLAN_ID not found: $result"
                    )
                    null
                }
            }
            else -> {
                log.error("failed to query subscription details: $result")
                null
            }
        }
    }

    suspend fun launchFlow(activity: Activity): PurchaseResult {
        val productDetailsParams =
            this.querySubscriptionDetails(this.connection.client)?.let {
                BillingFlowParams.ProductDetailsParams.newBuilder()
                    .setProductDetails(it.productDetails)
                    .setOfferToken(it.offerDetails.offerToken)
                    .build()
            } ?: return PurchaseResult.Failed
        return this.connection.purchaseUpdatesRx
            .onSubscription {
                val result =
                    // `launchBillingFlow` can only be called on the UI thread
                    withContext(Dispatchers.Main) {
                        this@BillingManager.connection.client.launchBillingFlow(
                            activity,
                            BillingFlowParams.newBuilder()
                                .setProductDetailsParamsList(listOf(productDetailsParams))
                                .build(),
                        )
                    }
                // This is the result of launching the flow, not of the purchase within the flow!
                when (result.responseCode) {
                    BillingClient.BillingResponseCode.OK -> {
                        log.info("launched billing flow successfully")
                    }
                    BillingClient.BillingResponseCode.USER_CANCELED -> {
                        log.info("user canceled billing flow")
                        this.emit(PurchaseResult.Canceled)
                    }
                    BillingClient.BillingResponseCode.ITEM_ALREADY_OWNED -> {
                        log.warn("user already owns item")
                        this.emit(PurchaseResult.AlreadyOwned)
                    }
                    else -> {
                        log.error("failed to launch billing flow: $result")
                        this.emit(PurchaseResult.Failed)
                    }
                }
            }
            // Wait for actual purchase result
            .firstOrNull()
            ?: run {
                log.error("purchase updates flow was empty")
                PurchaseResult.Failed
            }
    }

    fun destroy() {
        this.connection.destroy()
    }
}

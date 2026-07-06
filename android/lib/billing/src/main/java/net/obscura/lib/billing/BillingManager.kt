package net.obscura.lib.billing

import android.app.Activity
import android.content.Context
import com.android.billingclient.api.BillingClient
import com.android.billingclient.api.BillingFlowParams
import com.android.billingclient.api.ProductDetails
import com.android.billingclient.api.Purchase
import com.android.billingclient.api.QueryProductDetailsParams
import com.android.billingclient.api.QueryPurchasesParams
import com.android.billingclient.api.queryProductDetails
import kotlin.coroutines.resume
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.firstOrNull
import kotlinx.coroutines.flow.onSubscription
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import net.obscura.lib.util.Logger

private val log = Logger(BillingManager::class)

class BillingManager(context: Context) {
    sealed interface PurchaseResult {
        data class Completed(val purchaseTokens: List<String>) : PurchaseResult

        data object Canceled : PurchaseResult

        data object AlreadyOwned : PurchaseResult

        data object Failed : PurchaseResult
    }

    private val connection =
        BillingConnection(
            context,
        ) { result, purchases ->
            when (result.responseCode) {
                BillingClient.BillingResponseCode.OK -> {
                    if (purchases?.isNotEmpty() == true) {
                        PurchaseResult.Completed(purchases.map { it.purchaseToken })
                    } else {
                        log.error("purchase list null or empty despite a successful purchase")
                        PurchaseResult.Failed
                    }
                }
                BillingClient.BillingResponseCode.USER_CANCELED -> PurchaseResult.Canceled
                BillingClient.BillingResponseCode.ITEM_ALREADY_OWNED -> PurchaseResult.AlreadyOwned
                else -> {
                    log.error("purchase failed: $result")
                    PurchaseResult.Failed
                }
            }
        }

    private suspend fun querySubscriptionPurchases(): List<Purchase>? = suspendCancellableCoroutine { continuation ->
        this.connection.client.queryPurchasesAsync(
            QueryPurchasesParams.newBuilder().setProductType(BillingClient.ProductType.SUBS).build(),
        ) { result, purchases ->
            log.info("purchases response: $result $purchases")
            when (result.responseCode) {
                BillingClient.BillingResponseCode.OK -> {
                    if (continuation.isActive) {
                        continuation.resume(purchases)
                    }
                }
                else -> {
                    log.error("purchases response had unexpected billing result: $result")
                    if (continuation.isActive) {
                        continuation.resume(null)
                    }
                }
            }
        }
    }

    suspend fun fetchPurchaseTokens() = this.querySubscriptionPurchases()?.map { it.purchaseToken }

    private data class SubscriptionDetails(
        val productDetails: ProductDetails,
        val offerDetails: ProductDetails.SubscriptionOfferDetails,
    )

    private suspend fun querySubscriptionDetails(
        productId: String,
        basePlanId: String,
        offerId: String?,
    ): SubscriptionDetails? {
        val result =
            this.connection.client.queryProductDetails(
                QueryProductDetailsParams.newBuilder()
                    .setProductList(
                        listOf(
                            QueryProductDetailsParams.Product.newBuilder()
                                .setProductId(productId)
                                .setProductType(BillingClient.ProductType.SUBS)
                                .build()
                        )
                    )
                    .build()
            )
        return when (result.billingResult.responseCode) {
            BillingClient.BillingResponseCode.OK -> {
                val productDetails =
                    result.productDetailsList?.find { it.productId == productId }
                        ?: run {
                            log.error("product $productId not found: ${result.productDetailsList}")
                            return null
                        }
                log.info("product details: $productDetails")
                val offerDetails =
                    productDetails.subscriptionOfferDetails?.find {
                        it.basePlanId == basePlanId && it.offerId == offerId
                    }
                if (offerDetails != null) {
                    log.info("offer details: $offerDetails")
                    SubscriptionDetails(productDetails, offerDetails)
                } else {
                    if (offerId != null) {
                        log.error("offer $offerId not found: ${productDetails.subscriptionOfferDetails}")
                    } else {
                        log.error("base plan $basePlanId not found: ${productDetails.subscriptionOfferDetails}")
                    }
                    null
                }
            }
            else -> {
                log.error("failed to query subscription details: $result")
                null
            }
        }
    }

    suspend fun launchFlow(
        activity: Activity,
        productId: String,
        basePlanId: String,
        offerId: String?,
    ): PurchaseResult {
        val productDetailsParams =
            this.querySubscriptionDetails(productId, basePlanId, offerId)?.let {
                BillingFlowParams.ProductDetailsParams.newBuilder()
                    .setProductDetails(it.productDetails)
                    .setOfferToken(it.offerDetails.offerToken)
                    .build()
            } ?: return PurchaseResult.Failed
        return this.connection.purchaseUpdatesRx
            .onSubscription {
                val result =
                    // `launchBillingFlow` can only be called on the UI thread
                    withContext(Dispatchers.Main.immediate) {
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

    fun destroy() =
        runCatching { this.connection.destroy() }
            .onFailure { log.error("failed to end billing connection: ${it.message}") }
}

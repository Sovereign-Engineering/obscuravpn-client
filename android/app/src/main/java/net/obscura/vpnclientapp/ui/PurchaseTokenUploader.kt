package net.obscura.vpnclientapp.ui

import android.os.DeadObjectException
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.cancel
import kotlinx.coroutines.channels.ClosedReceiveChannelException
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import net.obscura.lib.util.BinaryExponentialBackoff
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.BillingFacade
import net.obscura.vpnclientapp.client.ManagerCmd
import net.obscura.vpnclientapp.client.errorCodeApiAssociateAccountConflict
import net.obscura.vpnclientapp.client.errorCodeApiRateLimitExceeded
import net.obscura.vpnclientapp.client.jsonConfig
import net.obscura.vpnclientapp.services.IObscuraVpnService

private val log = Logger(PurchaseTokenUploader::class)

class PurchaseTokenUploader(
    private val binder: IObscuraVpnService,
    private val billingFacade: BillingFacade,
) {
    private sealed interface AssociatePurchaseTokenResult {
        object Success : AssociatePurchaseTokenResult

        object Conflict : AssociatePurchaseTokenResult

        object RateLimit : AssociatePurchaseTokenResult

        object Retry : AssociatePurchaseTokenResult

        data class Fatal(val e: Throwable) : AssociatePurchaseTokenResult
    }

    private val scope = CoroutineScope(Dispatchers.IO)

    private suspend fun refreshPurchaseTokens() =
        try {
            log.debug("refreshing purchase tokens...")
            this.billingFacade.refreshPurchaseTokens()
        } catch (e: CancellationException) {
            log.debug("purchase token refresh job canceled: ${e.message}")
            throw e
        } catch (e: Throwable) {
            log.error("purchase token refresh job failed: ${e.message}", tr = e)
        }

    private suspend fun receivePurchaseToken(): String =
        try {
            log.debug("waiting for purchase tokens...")
            this.billingFacade.purchaseTokensRx.receive().also { log.info("received purchase token: $it") }
        } catch (e: CancellationException) {
            log.debug("purchase token channel closed: ${e.message}")
            throw e
        } catch (e: ClosedReceiveChannelException) {
            val message = "purchase token channel closed without cause: ${e.message}"
            log.debug(message)
            throw CancellationException(message)
        }

    private suspend fun associatePurchaseToken(purchaseToken: String): AssociatePurchaseTokenResult =
        try {
            log.debug("trying to associate purchase token with account: $purchaseToken")
            JsonFfiBroadcastReceiver.waitForResponse(
                    this.binder,
                    jsonConfig.encodeToString(ManagerCmd.ApiGoogleAssociateAccount(purchaseToken)),
                )
                .await()
            log.debug("associated purchase token with account")
            AssociatePurchaseTokenResult.Success
        } catch (e: CancellationException) {
            log.debug("account association job canceled: ${e.message}")
            throw e
        } catch (e: DeadObjectException) {
            log.error("binder is dead; giving up: ${e.message}")
            AssociatePurchaseTokenResult.Fatal(e)
        } catch (e: Throwable) {
            when (e) {
                errorCodeApiAssociateAccountConflict() -> {
                    log.error("purchase token already associated with another account")
                    AssociatePurchaseTokenResult.Conflict
                }
                errorCodeApiRateLimitExceeded() -> {
                    log.error("hit rate limit")
                    AssociatePurchaseTokenResult.RateLimit
                }
                else -> {
                    log.error("failed to associate purchase token with account; retrying: ${e.message}")
                    AssociatePurchaseTokenResult.Retry
                }
            }
        }

    private suspend fun uploadPurchaseToken(innerScope: CoroutineScope) {
        val purchaseToken = this.receivePurchaseToken()
        val backoff = BinaryExponentialBackoff()
        while (innerScope.isActive) {
            when (val result = this.associatePurchaseToken(purchaseToken)) {
                AssociatePurchaseTokenResult.Success,
                AssociatePurchaseTokenResult.Conflict -> {
                    break
                }
                AssociatePurchaseTokenResult.RateLimit -> {
                    backoff.maximize()
                    backoff.wait()
                }
                AssociatePurchaseTokenResult.Retry -> {
                    backoff.wait()
                }
                is AssociatePurchaseTokenResult.Fatal -> {
                    throw CancellationException("giving up after fatal error: ${result.e.message}")
                }
            }
        }
    }

    init {
        this.scope.launch {
            this@PurchaseTokenUploader.refreshPurchaseTokens()
            while (this.isActive) {
                this@PurchaseTokenUploader.uploadPurchaseToken(this)
            }
        }
    }

    fun cancel() {
        this.scope.cancel(CancellationException("purchase token upload canceled"))
    }
}

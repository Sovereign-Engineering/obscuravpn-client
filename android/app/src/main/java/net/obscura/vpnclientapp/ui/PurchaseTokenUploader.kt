package net.obscura.vpnclientapp.ui

import android.os.DeadObjectException
import kotlinx.coroutines.CancellationException
import net.obscura.lib.util.BinaryExponentialBackoff
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.client.ErrorCodeException
import net.obscura.vpnclientapp.client.ManagerCmd
import net.obscura.vpnclientapp.client.errorCodeApiAssociateAccountConflict
import net.obscura.vpnclientapp.client.errorCodeApiRateLimitExceeded
import net.obscura.vpnclientapp.client.errorCodeOther
import net.obscura.vpnclientapp.client.jsonConfig
import net.obscura.vpnclientapp.services.IObscuraVpnService

private val log = Logger("PurchaseTokenUploader")

private sealed interface AssociatePurchaseTokenResult {
    object Success : AssociatePurchaseTokenResult

    object RateLimit : AssociatePurchaseTokenResult

    object Retry : AssociatePurchaseTokenResult

    data class Fatal(val e: ErrorCodeException) : AssociatePurchaseTokenResult
}

private suspend fun associatePurchaseToken(
    binder: IObscuraVpnService,
    purchaseToken: String,
    promoCode: String?,
): AssociatePurchaseTokenResult =
    try {
        log.debug("trying to associate purchase token with account: $purchaseToken")
        JsonFfiBroadcastReceiver.waitForResponse(
                binder,
                jsonConfig.encodeToString(ManagerCmd.ApiGoogleAssociateAccount(purchaseToken, promoCode)),
            )
            .await()
        log.debug("associated purchase token with account")
        AssociatePurchaseTokenResult.Success
    } catch (e: CancellationException) {
        log.debug("account association job canceled: ${e.message}")
        throw e
    } catch (e: DeadObjectException) {
        log.error("binder is dead; giving up: ${e.message}")
        AssociatePurchaseTokenResult.Fatal(errorCodeOther())
    } catch (e: Throwable) {
        when (e) {
            errorCodeApiAssociateAccountConflict() -> {
                log.error("purchase token already associated with another account")
                AssociatePurchaseTokenResult.Fatal(errorCodeApiAssociateAccountConflict())
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

suspend fun uploadPurchaseToken(binder: IObscuraVpnService, purchaseToken: String, promoCode: String?) {
    val backoff = BinaryExponentialBackoff()
    // There's no need to check `CoroutineScope.isActive`, since `Deferred.await` guarantees prompt cancellation
    while (true) {
        when (val result = associatePurchaseToken(binder, purchaseToken, promoCode)) {
            AssociatePurchaseTokenResult.Success -> {
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
                throw result.e
            }
        }
    }
}

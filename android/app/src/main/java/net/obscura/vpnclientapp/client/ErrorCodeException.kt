package net.obscura.vpnclientapp.client

import androidx.annotation.Keep

// This `Keep` annotation is applied defensively to ensure that this class won't be stripped even if
// it's only constructed on the Rust side.
@Keep data class ErrorCodeException(val errorCode: String) : Exception(errorCode)

fun errorCodeOther() = ErrorCodeException("other")

fun errorCodePurchaseFailed() = ErrorCodeException("purchaseFailed")

fun errorCodePurchaseFailedAlreadyOwned() = ErrorCodeException("purchaseFailedAlreadyOwned")

fun errorCodeUnsupportedOnOS() = ErrorCodeException("errorUnsupportedOnOS")

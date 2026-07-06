package net.obscura.vpnclientapp.client

import androidx.annotation.Keep

// This `Keep` annotation is applied defensively to ensure that this class won't be stripped even if it's only
// constructed on the Rust side.
@Keep
data class ErrorCodeException(val errorCode: String) : Exception(errorCode) {
    companion object {
        fun apiAssociateAccountConflict() = ErrorCodeException("apiAssociateAccountConflict")

        fun apiRateLimitExceeded() = ErrorCodeException("apiRateLimitExceeded")

        fun other() = ErrorCodeException("other")

        fun playServicesDisabled() = ErrorCodeException("playServicesDisabled")

        fun playServicesMissing() = ErrorCodeException("playServicesMissing")

        fun playServicesUpdateRequired() = ErrorCodeException("playServicesUpdateRequired")

        fun playServicesUpdating() = ErrorCodeException("playServicesUpdating")

        fun purchaseFailed() = ErrorCodeException("purchaseFailed")

        fun purchaseFailedAlreadyOwned() = ErrorCodeException("purchaseFailedAlreadyOwned")

        fun legacyAlwaysOn() = ErrorCodeException("errorLegacyAlwaysOn")

        fun otherAppAlwaysOn() = ErrorCodeException("errorOtherAppAlwaysOn")

        fun permissionNotGranted() = ErrorCodeException("errorPermissionNotGranted")

        fun unsupportedOnOS() = ErrorCodeException("errorUnsupportedOnOS")
    }
}

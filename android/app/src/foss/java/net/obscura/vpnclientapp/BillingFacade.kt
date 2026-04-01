package net.obscura.vpnclientapp

import android.content.Context
import java.util.concurrent.CompletableFuture
import net.obscura.vpnclientapp.activities.MainActivity
import net.obscura.vpnclientapp.client.errorCodeUnsupportedOnOS

class BillingFacade(@Suppress("UNUSED_PARAMETER") context: Context) {
    fun launchFlow(@Suppress("UNUSED_PARAMETER") mainActivity: MainActivity): CompletableFuture<String> =
        CompletableFuture.failedFuture(errorCodeUnsupportedOnOS())

    fun destroy() = Unit
}

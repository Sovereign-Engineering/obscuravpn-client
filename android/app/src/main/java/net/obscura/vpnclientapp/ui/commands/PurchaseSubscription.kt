package net.obscura.vpnclientapp.ui.commands

import java.util.concurrent.CompletableFuture
import kotlinx.serialization.Serializable
import net.obscura.vpnclientapp.activities.MainActivity

@Serializable
class PurchaseSubscription {
    fun run(mainActivity: MainActivity): CompletableFuture<String> = mainActivity.billingFacade.launchFlow(mainActivity)
}

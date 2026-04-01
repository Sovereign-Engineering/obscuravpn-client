package net.obscura.vpnclientapp.ui

import android.content.Context
import dagger.Module
import dagger.Provides
import dagger.hilt.InstallIn
import dagger.hilt.android.ActivityRetainedLifecycle
import dagger.hilt.android.components.ActivityRetainedComponent
import dagger.hilt.android.qualifiers.ApplicationContext
import dagger.hilt.android.scopes.ActivityRetainedScoped
import net.obscura.vpnclientapp.BillingFacade

// Lifecycle/scope discussion:
// https://www.revenuecat.com/blog/engineering/hilt-sdk-lifecycle/
@Module
@InstallIn(ActivityRetainedComponent::class)
object BillingModule {
    @Provides
    @ActivityRetainedScoped
    fun provideBillingFacade(
        @ApplicationContext context: Context,
        lifecycle: ActivityRetainedLifecycle,
    ): BillingFacade {
        val billing = BillingFacade(context)
        lifecycle.addOnClearedListener { billing.destroy() }
        return billing
    }
}

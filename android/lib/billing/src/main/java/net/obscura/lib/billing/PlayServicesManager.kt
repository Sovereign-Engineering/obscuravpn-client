package net.obscura.lib.billing

import android.app.Activity
import android.content.Context
import com.google.android.gms.common.ConnectionResult
import com.google.android.gms.common.GoogleApiAvailability
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.tasks.await
import kotlinx.coroutines.withContext
import net.obscura.lib.util.Logger

private val log = Logger(PlayServicesManager::class)

object PlayServicesManager {
    sealed interface AvailabilityResult {
        data class Present(val inactiveReason: InactiveReason?) : AvailabilityResult {
            enum class InactiveReason {
                Disabled,
                UpdateRequired,
                Updating,
                Unknown,
            }
        }

        data object Missing : AvailabilityResult
    }

    fun AvailabilityResult.inactiveReason() = (this as? AvailabilityResult.Present)?.inactiveReason

    fun AvailabilityResult.isMissing() = this is AvailabilityResult.Missing

    private val instance = GoogleApiAvailability.getInstance()

    fun checkAvailability(context: Context): AvailabilityResult {
        val resultCode = this.instance.isGooglePlayServicesAvailable(context)
        // `isUserResolvableError` isn't useful for us, since it includes `SERVICE_MISSING` and `SERVICE_INVALID` but
        // excludes `SERVICE_UPDATING`.
        val result =
            when (resultCode) {
                ConnectionResult.SUCCESS -> AvailabilityResult.Present(null)
                ConnectionResult.SERVICE_DISABLED ->
                    AvailabilityResult.Present(AvailabilityResult.Present.InactiveReason.Disabled)
                ConnectionResult.SERVICE_VERSION_UPDATE_REQUIRED ->
                    AvailabilityResult.Present(AvailabilityResult.Present.InactiveReason.UpdateRequired)
                ConnectionResult.SERVICE_UPDATING ->
                    AvailabilityResult.Present(AvailabilityResult.Present.InactiveReason.Updating)
                ConnectionResult.SERVICE_MISSING,
                ConnectionResult.SERVICE_INVALID -> AvailabilityResult.Missing
                else -> {
                    log.warn("unexpected result code: $resultCode")
                    AvailabilityResult.Present(AvailabilityResult.Present.InactiveReason.Unknown)
                }
            }
        log.info("Play Services available: $resultCode -> $result")
        return result
    }

    suspend fun makeAvailable(activity: Activity) =
        try {
            log.info("attempting to make Play Services available")
            withContext(Dispatchers.Main.immediate) {
                this@PlayServicesManager.instance.makeGooglePlayServicesAvailable(activity).await()
            }
            true
        } catch (e: CancellationException) {
            throw e
        } catch (e: Throwable) {
            log.error("failed to make Play Services available: ${e.message}")
            false
        }
}

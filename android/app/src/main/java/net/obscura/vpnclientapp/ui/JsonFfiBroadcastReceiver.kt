package net.obscura.vpnclientapp.ui

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import java.util.concurrent.ConcurrentHashMap
import kotlin.random.Random
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.completeWith
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.services.IObscuraVpnService
import net.obscura.vpnclientapp.services.getJsonFfiExtras

private val log = Logger(JsonFfiBroadcastReceiver::class)

class JsonFfiBroadcastReceiver : BroadcastReceiver() {
    companion object {
        private val waiting by lazy { ConcurrentHashMap<Long, CompletableDeferred<String>>() }

        internal fun waitForResponse(
            binder: IObscuraVpnService,
            cmd: String,
        ) =
            CompletableDeferred<String>().also { job ->
                var id: Long
                do {
                    id = Random.nextLong()
                } while (this.waiting.putIfAbsent(id, job) != null)
                log.trace("job $id registered: $cmd")
                try {
                    binder.jsonFfi(id, cmd)
                } catch (e: Throwable) {
                    log.error("job $id failed: $e", tr = e)
                    this.waiting.remove(id)
                    job.completeExceptionally(e)
                }
            }
    }

    override fun onReceive(context: Context, intent: Intent) {
        val args = intent.getJsonFfiExtras()
        waiting.remove(args.id)?.completeWith(args.result)
            ?: run {
                log.error(
                    "job ${args.id} already completed or never registered (may be stale response from earlier process)"
                )
            }
    }
}

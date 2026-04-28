package net.obscura.vpnclientapp.ui

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.completeWith
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.services.IObscuraVpnService
import net.obscura.vpnclientapp.services.getJsonFfiExtras

private val log = Logger(JsonFfiBroadcastReceiver::class)

class JsonFfiBroadcastReceiver : BroadcastReceiver() {
    companion object {
        private val waiting by lazy { ConcurrentHashMap<Long, CompletableDeferred<String>>() }
        private val currentId = AtomicLong(0)

        internal fun waitForResponse(
            binder: IObscuraVpnService,
            cmd: String,
        ) =
            CompletableDeferred<String>().also { job ->
                val id = this.currentId.incrementAndGet()
                log.trace("job $id registered: $cmd")
                try {
                    binder.jsonFfi(id, cmd)
                    this.waiting[id] = job
                } catch (e: Throwable) {
                    log.error("job $id failed: $e", tr = e)
                    job.completeExceptionally(e)
                }
            }
    }

    override fun onReceive(context: Context, intent: Intent) {
        val args = intent.getJsonFfiExtras()
        waiting.remove(args.id)?.completeWith(args.result)
            ?: run { log.error("job ${args.id} already completed (or never registered)") }
    }
}

package net.obscura.vpnclientapp.services

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import java.util.concurrent.CompletableFuture
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.completeWith
import net.obscura.lib.util.Logger
import net.obscura.vpnclientapp.client.ErrorCodeException
import net.obscura.vpnclientapp.helpers.requireUIProcess

private val log = Logger(JsonFfiBroadcastReceiver::class)

// This handles both directions of IPC
class JsonFfiBroadcastReceiver : BroadcastReceiver() {
    companion object {
        private val waiting by lazy { ConcurrentHashMap<Long, CompletableDeferred<String>>() }
        private val currentId = AtomicLong(0)

        private const val EXTRA_ID = "id"
        private const val EXTRA_RESULT = "result"
        private const val EXTRA_EXCEPTION = "exception"

        fun waitForResponse(
            binder: IObscuraVpnService,
            cmd: String,
        ): CompletableDeferred<String> {
            val id = currentId.incrementAndGet()
            log.trace("job $id registered: $cmd")
            val job = CompletableDeferred<String>()
            try {
                binder.jsonFfi(id, cmd)
                this.waiting[id] = job
            } catch (e: Throwable) {
                log.error("job $id failed: $e", tr = e)
                job.completeExceptionally(e)
            }
            return job
        }

        internal fun broadcast(context: Context, id: Long, future: CompletableFuture<String>) {
            future.handle { result, exception ->
                context.sendBroadcast(
                    Intent(context, JsonFfiBroadcastReceiver::class.java).apply {
                        this.putExtra(EXTRA_ID, id)
                        if (exception != null) {
                            this.putExtra(EXTRA_EXCEPTION, exception.message)
                        } else {
                            this.putExtra(EXTRA_RESULT, result)
                        }
                    }
                )
            }
        }
    }

    override fun onReceive(context: Context, intent: Intent) {
        requireUIProcess()
        val id = intent.getLongExtra(EXTRA_ID, -1)
        val result = intent.getStringExtra(EXTRA_RESULT)
        val errorCode = intent.getStringExtra(EXTRA_EXCEPTION)
        waiting
            .remove(id)
            ?.completeWith(
                if (errorCode != null) {
                    log.trace("job $id completed with failure: $errorCode")
                    Result.failure(ErrorCodeException(errorCode))
                } else {
                    log.trace("job $id completed with success: $result")
                    Result.success(result ?: "null")
                }
            ) ?: run { log.error("job $id already completed (or never registered)") }
    }
}

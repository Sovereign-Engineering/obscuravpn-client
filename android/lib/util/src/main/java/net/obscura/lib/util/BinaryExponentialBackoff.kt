package net.obscura.lib.util

import kotlin.time.Duration
import kotlin.time.Duration.Companion.seconds
import kotlinx.coroutines.delay

class BinaryExponentialBackoff(private val base: Duration = 1.seconds, private val maxPow: Int = 6) {
    private var curPow = 0

    fun reset() {
        this.curPow = 0
    }

    fun maximize() {
        this.curPow = this.maxPow
    }

    suspend fun wait() {
        delay(this.base.times(1 shl this.curPow))
        this.curPow = (this.curPow + 1).coerceAtMost(this.maxPow)
    }
}

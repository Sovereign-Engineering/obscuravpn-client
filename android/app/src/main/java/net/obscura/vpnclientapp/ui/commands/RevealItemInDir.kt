package net.obscura.vpnclientapp.ui.commands

import java.util.concurrent.CompletableFuture
import kotlinx.serialization.Serializable
import net.obscura.vpnclientapp.client.errorCodeUnsupportedOnOS

@Serializable
data class RevealItemInDir(val path: String) {
    fun run() = CompletableFuture<String>().apply { this.completeExceptionally(errorCodeUnsupportedOnOS()) }
}

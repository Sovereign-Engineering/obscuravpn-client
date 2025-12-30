package net.obscura.vpnclientapp.helpers

import java.util.concurrent.CompletableFuture

fun completedJsonNullFuture() = CompletableFuture.completedFuture("null")!!

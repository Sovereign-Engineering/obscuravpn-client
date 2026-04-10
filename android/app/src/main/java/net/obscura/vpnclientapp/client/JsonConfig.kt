package net.obscura.vpnclientapp.client

import kotlinx.serialization.json.ClassDiscriminatorMode
import kotlinx.serialization.json.Json

val jsonConfig = Json {
    this.classDiscriminatorMode = ClassDiscriminatorMode.NONE
    this.encodeDefaults = true
    this.ignoreUnknownKeys = true
}

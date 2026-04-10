package net.obscura.lib.util

import kotlinx.serialization.KSerializer
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonTransformingSerializer
import kotlinx.serialization.json.buildJsonObject
import kotlinx.serialization.json.jsonObject

open class ExternallyTaggedEnumVariantSerializer<T>(val tag: String, serializer: KSerializer<T>) :
    JsonTransformingSerializer<T>(serializer) {
    override fun transformDeserialize(element: JsonElement): JsonElement = checkNotNull(element.jsonObject[this.tag])

    override fun transformSerialize(element: JsonElement): JsonElement = buildJsonObject {
        this.put(this@ExternallyTaggedEnumVariantSerializer.tag, element)
    }
}

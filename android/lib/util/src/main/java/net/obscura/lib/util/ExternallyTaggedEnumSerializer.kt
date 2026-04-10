package net.obscura.lib.util

import kotlin.reflect.KClass
import kotlinx.serialization.KSerializer
import kotlinx.serialization.json.JsonContentPolymorphicSerializer
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.jsonObject

open class ExternallyTaggedEnumSerializer<T : Any>(
    private val baseClass: KClass<T>,
    private val variants: List<ExternallyTaggedEnumVariantSerializer<out T>>,
) : JsonContentPolymorphicSerializer<T>(baseClass) {
    override fun selectDeserializer(element: JsonElement): KSerializer<out T> =
        this.variants.find { it.tag in element.jsonObject } ?: error("invalid `${this.baseClass.simpleName}` variant")
}

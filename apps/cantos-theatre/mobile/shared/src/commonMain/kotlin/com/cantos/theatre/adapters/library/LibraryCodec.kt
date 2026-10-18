package com.cantos.theatre.adapters.library

import com.cantos.theatre.adapters.library.wire.librarySchemaJson

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonArray
import kotlinx.serialization.json.JsonElement
import kotlinx.serialization.json.JsonNull
import kotlinx.serialization.json.JsonObject
import kotlinx.serialization.json.JsonPrimitive
import kotlinx.serialization.json.booleanOrNull
import kotlinx.serialization.json.decodeFromJsonElement
import kotlinx.serialization.json.int
import kotlinx.serialization.json.jsonArray
import kotlinx.serialization.json.jsonObject
import kotlinx.serialization.json.jsonPrimitive
import kotlinx.serialization.json.long
import kotlinx.serialization.json.longOrNull

/** Generated schema is the authority, including ranges/patterns omitted by Kotlin serializers. */
internal object LibraryCodec {
    val json = Json { ignoreUnknownKeys = false; explicitNulls = true; classDiscriminator = "resource" }
    private val definitions = (json.parseToJsonElement(librarySchemaJson) as JsonObject)["\$defs"] as JsonObject
    fun valid(type: String, value: JsonElement): Boolean = validate(definitions.getValue(type) as JsonObject, value)
    private fun validate(schema: JsonObject, value: JsonElement): Boolean {
        schema["\$ref"]?.jsonPrimitive?.content?.let { return valid(it.substringAfterLast('/'), value) }
        schema["anyOf"]?.jsonArray?.let { return it.any { child -> validate(child.jsonObject, value) } }
        schema["oneOf"]?.jsonArray?.let { return it.count { child -> validate(child.jsonObject, value) } == 1 }
        schema["enum"]?.jsonArray?.let { if (value !in it) return false }
        return when (schema["type"]?.jsonPrimitive?.content) {
            "null" -> value == JsonNull
            "object" -> {
                val obj = value as? JsonObject ?: return false
                val properties = schema["properties"]?.jsonObject ?: return false
                val required = schema["required"]?.jsonArray?.map { it.jsonPrimitive.content }?.toSet() ?: emptySet()
                obj.keys.containsAll(required) && (schema["additionalProperties"] != JsonPrimitive(false) || obj.keys.all { it in properties }) &&
                    obj.all { (key, item) -> properties[key]?.let { validate(it.jsonObject, item) } ?: true }
            }
            "array" -> value is JsonArray && value.size <= (schema["maxItems"]?.jsonPrimitive?.int ?: Int.MAX_VALUE) && value.all { validate(schema.getValue("items").jsonObject, it) }
            "string" -> value is JsonPrimitive && value.isString && (schema["pattern"]?.jsonPrimitive?.content?.let { Regex(it).matches(value.content) } ?: true)
            "boolean" -> value is JsonPrimitive && !value.isString && value.booleanOrNull != null
            "integer" -> {
                val primitive = value as? JsonPrimitive ?: return false
                val number = primitive.takeUnless { it.isString }?.longOrNull ?: return false
                number >= (schema["minimum"]?.jsonPrimitive?.long ?: Long.MIN_VALUE) && number <= (schema["maximum"]?.jsonPrimitive?.long ?: Long.MAX_VALUE)
            }
            else -> false
        }
    }
    inline fun <reified T> decode(type: String, raw: String): T? = try {
        val value = json.parseToJsonElement(raw)
        if (valid(type, value)) json.decodeFromJsonElement<T>(value) else null
    } catch (_: IllegalArgumentException) { null }
}

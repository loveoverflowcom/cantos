package com.cantos.theatre.adapters.http

import kotlinx.serialization.json.Json
import kotlinx.serialization.json.JsonPrimitive

/** JsonObject otherwise keeps only the last duplicate, including escaped key spellings. */
internal fun repeatedTopLevelKeys(body: String): Boolean {
    val keys = mutableSetOf<String>()
    var depth = 0
    var keyExpected = false
    var index = 0
    while (index < body.length) {
        when (body[index]) {
            '{', '[' -> { depth += 1; if (depth == 1) keyExpected = true }
            '}', ']' -> depth -= 1
            ',' -> if (depth == 1) keyExpected = true
            ':' -> if (depth == 1) keyExpected = false
            '"' -> {
                val start = index
                index += 1
                while (index < body.length && body[index] != '"') {
                    if (body[index] == '\\') index += 1
                    index += 1
                }
                if (depth == 1 && keyExpected) {
                    val key = (Json.parseToJsonElement(body.substring(start, index + 1)) as JsonPrimitive).content
                    if (!keys.add(key)) return true
                }
            }
        }
        index += 1
    }
    return false
}

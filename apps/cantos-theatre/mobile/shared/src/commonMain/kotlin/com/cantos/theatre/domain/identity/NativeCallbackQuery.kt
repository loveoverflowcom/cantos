package com.cantos.theatre.domain.identity

/**
 * Check raw query field names before a platform decoder reads their values. Android Uri
 * deduplicates decoded names but searches values by their canonical encoded name, so aliases
 * such as `state` and `%73tate` must be refused here. Values remain platform-decoded; this
 * guard establishes neither a valid pending state nor an authenticated account.
 */
internal fun hasCanonicalNativeCallbackQuery(encodedQuery: String?): Boolean {
    if (encodedQuery.isNullOrEmpty() || encodedQuery.length > 16_384) return false
    val allowed = setOf("state", "code", "error", "error_description")
    val names = mutableSetOf<String>()
    for (field in encodedQuery.split('&')) {
        val separator = field.indexOf('=')
        if (separator <= 0) return false
        val name = field.substring(0, separator)
        if (name !in allowed || !names.add(name)) return false
    }
    return "state" in names
}

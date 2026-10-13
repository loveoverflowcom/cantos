package com.cantos.theatre.domain.identity

class NativeClientConfiguration private constructor(val apiOrigin: String, val identity: NativeConfiguration) {
    companion object {
        fun create(apiOrigin: String, issuer: String): NativeClientConfiguration? {
            val match = Regex("https://([a-z0-9.-]+)(?::([0-9]{1,5}))?").matchEntire(apiOrigin) ?: return null
            val host = match.groupValues[1]
            val port = match.groupValues[2]
            if (host.startsWith('.') || host.endsWith('.') || host.contains("..") ||
                (port.isNotEmpty() && port.toInt() !in 1..65535)) return null
            val identity = NativeConfiguration.create(issuer, "com.cantos.theatre:/oauth2/callback") ?: return null
            return NativeClientConfiguration(apiOrigin, identity)
        }
    }
}

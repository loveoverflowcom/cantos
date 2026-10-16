package com.cantos.theatre.adapters.http

/** What a host's configured API origin means for composing the live public catalog. */
sealed interface CatalogOriginSelection {
    data class Configured(val origin: ApiOrigin) : CatalogOriginSelection
    /** Nothing configured: the host keeps its explicit unavailable state. */
    data object NotConfigured : CatalogOriginSelection
    /** A value was configured but is not admissible for this build; fail closed, never fall back. */
    data object Rejected : CatalogOriginSelection
}

/**
 * Release builds admit only `https://` origins. [allowLoopbackHttp] is true only for a debug
 * build, where `http://` is further limited to `127.0.0.1` and `localhost` (for example
 * `adb reverse tcp:8080 tcp:8080`): exactly the hosts the debug network security config and
 * `cargo xtask theatre-host android-debug` permit. [ApiOrigin] also parses `[::1]`, but Android
 * would block that cleartext, so it is refused here. The value is used exactly as configured.
 */
fun selectCatalogOrigin(raw: String?, allowLoopbackHttp: Boolean): CatalogOriginSelection {
    if (raw.isNullOrEmpty()) return CatalogOriginSelection.NotConfigured
    val origin = try {
        ApiOrigin.parse(raw)
    } catch (_: IllegalArgumentException) {
        return CatalogOriginSelection.Rejected
    }
    if (!origin.isHttps) {
        val host = raw.removePrefix("http://").substringBefore(':').lowercase()
        if (!allowLoopbackHttp || host !in CleartextLoopbackHosts) return CatalogOriginSelection.Rejected
    }
    return CatalogOriginSelection.Configured(origin)
}

/** Must equal the debug network security config's cleartext domains. */
internal val CleartextLoopbackHosts = setOf("127.0.0.1", "localhost")
